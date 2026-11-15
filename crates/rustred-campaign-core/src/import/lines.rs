//! Raw scan of a `table_dump.v1` file: every line is classified (known kinds only), every rule record keeps its byte
//! offset, length, ordinal and `record_sha256` (plain sha256 hex of the raw line WITHOUT its newline: rr_proof's answer
//! echo, coord-ec2-instantiate.md s.2c). The domain fields are parsed by the core reader (`record::RecordReader`, one
//! line at a time), `domain_bounds` included (lane partfix4: one reader, one meaning, `domain::bound_branches`); the
//! few top-level fields the core reader does not keep (`origin`, `origin_index`, the span of `rhs`) are read here with a
//! string-aware top-level scanner (no field of a nested object is ever taken).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Cursor};
use std::path::Path;

use serde_json::Value;

use super::error::{ImportError, TableRefusal, io_err};
use crate::closure::sha256::Sha256;
use crate::record::stream::{StreamOptions, StreamedLine, StreamedRhs, stream_line};
use crate::record::{
    ReadLimits, ReadOptions, RecordError, RecordFormat, RecordReader, RhsMode, RuleRecord,
    SYM_KIND, SYM_ZERO_KIND, SymRecord, TABLE_LINE_KINDS, TableHeader, TableLine, parse_sym_line,
    read_until_limited,
};

/// Line kinds a table file may contain (the one closed list, `record::TABLE_LINE_KINDS`); anything else is a refusal.
pub const KNOWN_KINDS: &[&str] = TABLE_LINE_KINDS;

/// Non-rule kinds whose CONTENT the import reads (kept as payload in the table index; interfaces/import.md 18).
pub const CONTENT_KINDS: &[&str] = &[
    "residual",
    "composite_terminal_ledger",
    "composite_assembly_complete",
    "assembly_end",
    "assembly_provenance",
];

/// A `composite_terminal_ledger` record as written (checked by [`check_markers`]).
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerRecord {
    pub line: usize,
    pub keys: Option<Value>,
    pub count: Option<Value>,
}

/// A completeness marker (`composite_assembly_complete` / `assembly_end`) as written.
#[derive(Clone, Debug, PartialEq)]
pub struct CompletionMarker {
    pub kind: String,
    pub line: usize,
    pub complete: Option<Value>,
    pub rules: Option<Value>,
    pub residuals: Option<Value>,
    pub expected_lp: Option<Value>,
}

/// The records of a table that are not rules but state something about the table (interfaces/import.md 18).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TableMarkers {
    pub ledgers: Vec<LedgerRecord>,
    pub completions: Vec<CompletionMarker>,
    /// `assembly_provenance` records: (line, `one_physical_order` as written).
    pub provenance: Vec<(usize, Option<Value>)>,
    /// `residual` records: (line, key text).
    pub residuals: Vec<(usize, String)>,
}

/// Provenance of a `symface` slice record (coord-pole-engine.md 9c): read, never interpreted.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct SymfaceRef {
    pub schema: Option<String>,
    /// The member `face` verbatim (`{"dotted": [..], "numerators": [..]}`).
    pub face: Value,
    /// `element.index`.
    pub element: Option<i64>,
    /// The member `slice` verbatim.
    pub slice: Value,
}

/// The most values a `domain_bounds` entry may exclude on a FREE slot: the ONE constant
/// [`crate::domain::ENGINE_MAX_BOUND_EXCLUSIONS`] (rustred-core node/bounds.rs `MAX_EXCLUDED_PER_SLOT`). A bound on a
/// FIXED slot is a comparison with the fixed value and has no limit on either side.
pub const MAX_BOUND_EXCLUSIONS_PER_SLOT: i64 = crate::domain::ENGINE_MAX_BOUND_EXCLUSIONS;

/// `domain_bounds` as written (coord-bank-tables.md s.10): read by the core reader (`record::parse_line`), meaning
/// [`crate::domain::bound_branches`] (lane partfix4: was a second parser here and a second reading in load.rs).
pub use crate::record::DomainBounds;

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
    /// Byte span of the `rhs` value inside the line as read (informational; the index replaces it).
    pub rhs_span: Option<(usize, usize)>,
    /// A `sym` record (one-term symmetry rule): `rec` is its rule-record form ([`SymRecord::as_rule_record`]).
    pub sym: Option<SymRecord>,
    /// The `symface` provenance member of a slice record.
    pub symface: Option<SymfaceRef>,
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
    /// Ledger, completeness and provenance markers, residual keys.
    pub markers: TableMarkers,
    /// Number of `rule` lines and of `sym` lines of the file (kept or not).
    pub rule_lines: usize,
    pub sym_lines: usize,
}

/// Scan a table file. `want_rhs`: collect the rhs keys of the records (bodies). `keep(ordinal, label)` decides which
/// records are kept in memory (all are hashed and counted). No per-record limit ([`scan_table_with`]).
pub fn scan_table(
    source: &str,
    path: &Path,
    want_rhs: bool,
    keep: &dyn Fn(usize, Option<i64>) -> bool,
) -> Result<RawTable, ImportError> {
    scan_table_with(source, path, want_rhs, keep, ReadLimits::unlimited())
}

/// [`scan_table`] under per-record memory limits (lines above `limits.whole_line_max` are streamed).
pub fn scan_table_with(
    source: &str,
    path: &Path,
    want_rhs: bool,
    keep: &dyn Fn(usize, Option<i64>) -> bool,
    limits: ReadLimits,
) -> Result<RawTable, ImportError> {
    let mut records = vec![];
    let mut t = scan_table_each_with(
        source,
        path,
        want_rhs,
        keep,
        &mut |r| {
            records.push(r);
            Ok(())
        },
        limits,
    )?;
    t.records = records;
    Ok(t)
}

/// The header of a table file, read from its first non-blank line only (a file whose first line is not a header is
/// refused). Used for the frame: never a whole-file scan (tables can be tens of GB).
pub fn read_header(source: &str, path: &Path) -> Result<TableHeader, ImportError> {
    let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = BufReader::with_capacity(1 << 16, f);
    let mut buf: Vec<u8> = Vec::new();
    let mut line_no = 0usize;
    loop {
        buf.clear();
        let n = rd
            .read_until(b'\n', &mut buf)
            .map_err(|e| io_err(path, e))?;
        if n == 0 {
            break;
        }
        line_no += 1;
        if buf.iter().all(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let body = buf.strip_suffix(b"\n").unwrap_or(&buf);
        let lerr = |msg: String| ImportError::Line {
            source: source.to_string(),
            path: path.display().to_string(),
            line: line_no,
            msg,
        };
        let parsed = RecordReader::new(Cursor::new(body), ReadOptions { rhs: RhsMode::Skip })
            .next()
            .ok_or_else(|| lerr("empty line".into()))?
            .map_err(|e| lerr(e.to_string()))?;
        return match parsed {
            TableLine::Header(h) => Ok(*h),
            _ => Err(lerr("the first line is not a header".into())),
        };
    }
    Err(ImportError::Line {
        source: source.to_string(),
        path: path.display().to_string(),
        line: 0,
        msg: "file without header".into(),
    })
}

/// [`scan_table`] that hands every kept record to `visit` as soon as it is read (bounded memory: one record at a
/// time); the returned table has no records. No per-record limit ([`scan_table_each_with`]).
pub fn scan_table_each(
    source: &str,
    path: &Path,
    want_rhs: bool,
    keep: &dyn Fn(usize, Option<i64>) -> bool,
    visit: &mut dyn FnMut(RawRecord) -> Result<(), ImportError>,
) -> Result<RawTable, ImportError> {
    scan_table_each_with(source, path, want_rhs, keep, visit, ReadLimits::unlimited())
}

/// A [`BufRead`] over the bytes already read of a line, then the rest of the input.
struct PrefixRead<'a> {
    prefix: &'a [u8],
    pos: usize,
    inner: &'a mut dyn BufRead,
}

impl std::io::Read for PrefixRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let b = self.fill_buf()?;
        let n = b.len().min(out.len());
        out[..n].copy_from_slice(&b[..n]);
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for PrefixRead<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        if self.pos < self.prefix.len() {
            Ok(&self.prefix[self.pos..])
        } else {
            self.inner.fill_buf()
        }
    }
    fn consume(&mut self, n: usize) {
        if self.pos < self.prefix.len() {
            self.pos += n;
        } else {
            self.inner.consume(n);
        }
    }
}

/// [`scan_table_each`] under per-record memory limits: a line longer than `limits.whole_line_max` is STREAMED
/// ([`stream_line`]: hashed while read, the compact line parsed as any line), never held whole.
pub fn scan_table_each_with(
    source: &str,
    path: &Path,
    want_rhs: bool,
    keep: &dyn Fn(usize, Option<i64>) -> bool,
    visit: &mut dyn FnMut(RawRecord) -> Result<(), ImportError>,
    limits: ReadLimits,
) -> Result<RawTable, ImportError> {
    let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = BufReader::with_capacity(1 << 20, f);
    let mut file_h = Sha256::new();
    let mut sc = LineScanner::new(
        source,
        path,
        if want_rhs {
            RhsMode::KeysOnly
        } else {
            RhsMode::Skip
        },
        true,
    )
    .with_limits(limits);
    let mut buf: Vec<u8> = Vec::with_capacity(1 << 16);
    let (mut offset, mut line_no) = (0u64, 0usize);
    loop {
        buf.clear();
        let (n, complete) =
            read_until_limited(&mut rd, &mut buf, limits.whole_line_max.saturating_add(1))
                .map_err(|e| io_err(path, e))?;
        if n == 0 {
            break;
        }
        line_no += 1;
        let start = offset;
        if complete {
            file_h.update(&buf);
            offset += n as u64;
            let body = line_body(&buf);
            if let Fed::Rule(r) = sc.feed(body, line_no, start, body.len() as u64, None, keep)? {
                visit(*r)?;
            }
            continue;
        }
        // a long line: streamed
        let mut line_h = Sha256::new();
        let opts = sc.stream_options();
        let sl = {
            let mut pr = PrefixRead {
                prefix: &buf,
                pos: 0,
                inner: &mut rd,
            };
            stream_line(
                &mut pr,
                &opts,
                &mut |seg| {
                    file_h.update(seg);
                    line_h.update(seg);
                },
                None,
            )
            .map_err(|e| sc.record_error(line_no, e))?
        };
        if sl.newline {
            file_h.update(b"\n");
        }
        offset += sl.bytes + u64::from(sl.newline);
        let sha = (!sl.blank).then(|| line_h.finish_hex());
        if let Fed::Rule(r) = sc.feed_streamed(&sl, line_no, start, sha, keep)? {
            visit(*r)?;
        }
    }
    sc.finish(file_h.finish_hex(), offset)
}

/// The line without its trailing newline.
pub fn line_body(buf: &[u8]) -> &[u8] {
    if buf.last() == Some(&b'\n') {
        &buf[..buf.len() - 1]
    } else {
        buf
    }
}

/// What one fed line was.
pub enum Fed {
    Blank,
    Header,
    Chart,
    /// A known non-rule kind (counted).
    Other(String),
    /// A rule record that `keep` accepted.
    Rule(Box<RawRecord>),
    /// A rule record that `keep` refused (counted in the ordinals, not returned).
    RuleNotKept,
}

/// Per-line state of a table scan, shared by the direct file scan ([`scan_table_each`]) and the table index
/// ([`super::index`]): the same parse, the same refusals, the same record fields whatever the line's origin.
pub struct LineScanner<'a> {
    source: &'a str,
    path: &'a Path,
    opts: ReadOptions,
    rhs_digest: bool,
    limits: ReadLimits,
    header: Option<TableHeader>,
    charts: Vec<(String, Vec<String>)>,
    kinds: BTreeMap<String, usize>,
    n_records: usize,
    ordinal: usize,
    markers: TableMarkers,
    rule_lines: usize,
    sym_lines: usize,
}

impl<'a> LineScanner<'a> {
    /// `rhs_digest`: compute `rhs_sha256` (the raw rhs text digest, needed by supplied relations only).
    pub fn new(source: &'a str, path: &'a Path, rhs: RhsMode, rhs_digest: bool) -> Self {
        LineScanner {
            source,
            path,
            opts: ReadOptions { rhs },
            rhs_digest,
            limits: ReadLimits::unlimited(),
            header: None,
            charts: vec![],
            kinds: BTreeMap::new(),
            n_records: 0,
            ordinal: 0,
            markers: TableMarkers::default(),
            rule_lines: 0,
            sym_lines: 0,
        }
    }
    /// The scanner under per-record memory limits (default: unlimited).
    pub fn with_limits(mut self, limits: ReadLimits) -> Self {
        self.limits = limits;
        self
    }
    /// The limits in force.
    pub fn limits(&self) -> ReadLimits {
        self.limits
    }
    /// Options of [`stream_line`] for a line this scanner will be fed ([`LineScanner::feed_streamed`]).
    pub fn stream_options(&self) -> StreamOptions {
        StreamOptions {
            rhs: self.opts.rhs,
            rhs_digest: self.rhs_digest,
            budget: self.limits.record_budget,
            line: 1,
        }
    }
    /// The import error of a reader error on table line `line_no` (a record too large for the budget is a typed
    /// refusal of the table; any other reader error is the line error the in-memory path gives).
    pub fn record_error(&self, line_no: usize, e: RecordError) -> ImportError {
        match e {
            RecordError::TooLarge {
                what,
                bytes,
                budget,
                ..
            } => self.refused(
                line_no,
                TableRefusal::RecordTooLarge {
                    record: None,
                    what,
                    bytes,
                    budget,
                },
            ),
            RecordError::Io(err) => io_err(self.path, err),
            e => self.lerr(line_no, e.to_string()),
        }
    }
    fn whole_needed(&self, line_no: usize, kind: &str, bytes: u64) -> ImportError {
        self.refused(
            line_no,
            TableRefusal::RecordTooLarge {
                record: None,
                what: format!(
                    "a {kind} line with a top-level rhs array (read whole: longer than the whole-line limit {})",
                    self.limits.whole_line_max
                ),
                bytes,
                budget: self.limits.record_budget,
            },
        )
    }
    fn refused(&self, line: usize, reason: TableRefusal) -> ImportError {
        ImportError::Refused {
            source: self.source.to_string(),
            path: self.path.display().to_string(),
            line,
            reason: Box::new(reason),
        }
    }
    fn lerr(&self, line: usize, msg: String) -> ImportError {
        ImportError::Line {
            source: self.source.to_string(),
            path: self.path.display().to_string(),
            line,
            msg,
        }
    }
    /// Count one known non-rule line kind without its text (index reader).
    /// A rule / `sym` record after the `end` record (lane partfix4): the file is not what its end record closes.
    fn refuse_after_end(&self, line_no: usize, what: &str) -> Result<(), ImportError> {
        if self.kinds.contains_key("end") {
            return Err(self.refused(
                line_no,
                TableRefusal::Incomplete {
                    msg: format!("a {what} record after the end record"),
                },
            ));
        }
        Ok(())
    }
    pub fn count_kind(&mut self, line_no: usize, kind: &str) -> Result<(), ImportError> {
        if kind == SYM_ZERO_KIND {
            return Err(self.refused(line_no, TableRefusal::SymZero));
        }
        if CONTENT_KINDS.contains(&kind) || kind == SYM_KIND {
            return Err(self.lerr(
                line_no,
                format!("line kind {kind:?} needs its content (index without payload)"),
            ));
        }
        if !KNOWN_KINDS.contains(&kind) {
            return Err(self.lerr(line_no, format!("line kind {kind:?} is not a table line")));
        }
        *self.kinds.entry(kind.to_string()).or_default() += 1;
        Ok(())
    }
    /// One line. `body`: the line without its newline (for an index entry: the stored payload); `bytes`: the
    /// ORIGINAL line length without newline; `sha`: the original line's sha256 hex when already known, else it is
    /// computed from `body`.
    pub fn feed(
        &mut self,
        body: &[u8],
        line_no: usize,
        offset: u64,
        bytes: u64,
        sha: Option<String>,
        keep: &dyn Fn(usize, Option<i64>) -> bool,
    ) -> Result<Fed, ImportError> {
        self.feed_inner(body, line_no, offset, bytes, sha, keep, None)
    }

    /// One STREAMED line ([`stream_line`] with [`LineScanner::stream_options`]): the compact line is fed as any line;
    /// when its `rhs` value was replaced, the record takes the streamed rhs (keys, span, digest, a deferred key-grammar
    /// error) instead. `sha`: the original line's sha256 hex (None for a blank line).
    pub fn feed_streamed(
        &mut self,
        sl: &StreamedLine,
        line_no: usize,
        offset: u64,
        sha: Option<String>,
        keep: &dyn Fn(usize, Option<i64>) -> bool,
    ) -> Result<Fed, ImportError> {
        if sl.blank {
            return Ok(Fed::Blank);
        }
        self.feed_inner(
            &sl.compact,
            line_no,
            offset,
            sl.bytes,
            sha,
            keep,
            sl.rhs.as_ref(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn feed_inner(
        &mut self,
        body: &[u8],
        line_no: usize,
        offset: u64,
        bytes: u64,
        sha: Option<String>,
        keep: &dyn Fn(usize, Option<i64>) -> bool,
        streamed: Option<&StreamedRhs>,
    ) -> Result<Fed, ImportError> {
        if body.iter().all(|c| c.is_ascii_whitespace()) {
            return Ok(Fed::Blank);
        }
        let parsed = RecordReader::new(Cursor::new(body), self.opts)
            .next()
            .ok_or_else(|| self.lerr(line_no, "empty line".into()))?
            .map_err(|e| self.lerr(line_no, e.to_string()))?;
        match parsed {
            // a line whose content is kept whole (header, chart, content kinds) cannot lose its rhs value
            TableLine::Header(_) | TableLine::Chart { .. } if streamed.is_some() => {
                Err(self.whole_needed(line_no, "header or chart", bytes))
            }
            TableLine::Other { kind }
                if streamed.is_some() && CONTENT_KINDS.contains(&kind.as_str()) =>
            {
                Err(self.whole_needed(line_no, &kind, bytes))
            }
            TableLine::Header(h) => {
                if self.header.is_some()
                    || self.n_records > 0
                    || self.ordinal > 0
                    || !self.charts.is_empty()
                {
                    return Err(
                        self.lerr(line_no, "a second header or a header after records".into())
                    );
                }
                self.header = Some(*h);
                Ok(Fed::Header)
            }
            TableLine::Chart { chart, guards } => {
                *self.kinds.entry("chart".into()).or_default() += 1;
                self.charts.push((chart, guards));
                Ok(Fed::Chart)
            }
            TableLine::Other { kind } if kind == SYM_KIND => {
                if self.header.is_none() {
                    return Err(self.lerr(line_no, "sym record before the header".into()));
                }
                self.refuse_after_end(line_no, "sym")?;
                let sym = parse_sym_line(body, line_no)
                    .map_err(|e| self.refused(line_no, TableRefusal::Sym { msg: e.to_string() }))?;
                if sym.sign == "key" {
                    return Err(self.refused(
                        line_no,
                        TableRefusal::Sym {
                            msg: "sign `key` (a flipped free slot: impossible with squared propagators)"
                                .into(),
                        },
                    ));
                }
                self.sym_lines += 1;
                *self.kinds.entry(SYM_KIND.into()).or_default() += 1;
                let this = self.ordinal;
                self.ordinal += 1;
                if !keep(this, Some(sym.index)) {
                    return Ok(Fed::RuleNotKept);
                }
                let sha = match sha {
                    Some(s) => s,
                    None => {
                        let mut h = Sha256::new();
                        h.update(body);
                        h.finish_hex()
                    }
                };
                self.n_records += 1;
                Ok(Fed::Rule(Box::new(RawRecord {
                    ordinal: this,
                    line_no,
                    offset,
                    bytes,
                    sha256: sha,
                    rec: sym.as_rule_record(self.opts.rhs == RhsMode::KeysOnly),
                    domain_bounds: None,
                    origin: None,
                    origin_index: None,
                    rhs_sha256: None,
                    record_kind: Some(SYM_KIND.into()),
                    rhs_span: None,
                    sym: Some(sym),
                    symface: None,
                })))
            }
            TableLine::Other { kind } if CONTENT_KINDS.contains(&kind.as_str()) => {
                let v: Value = serde_json::from_slice(body)
                    .map_err(|e| self.lerr(line_no, format!("{kind}: {e}")))?;
                let g = |k: &str| v.get(k).cloned();
                match kind.as_str() {
                    "residual" => {
                        // counted whatever its key's form; only an `I(..)` text of the arity can become a declared
                        // terminal (`table_terminals: ledger_and_residuals` refuses any other)
                        let key = match v.get("key") {
                            Some(Value::String(t)) => t.clone(),
                            Some(other) => other.to_string(),
                            None => String::new(),
                        };
                        self.markers.residuals.push((line_no, key));
                    }
                    "composite_terminal_ledger" => self.markers.ledgers.push(LedgerRecord {
                        line: line_no,
                        keys: g("keys"),
                        count: g("count"),
                    }),
                    "assembly_provenance" => self
                        .markers
                        .provenance
                        .push((line_no, g("one_physical_order"))),
                    _ => self.markers.completions.push(CompletionMarker {
                        kind: kind.clone(),
                        line: line_no,
                        complete: g("complete"),
                        rules: g("rules"),
                        residuals: g("residuals"),
                        expected_lp: g("expected_LP"),
                    }),
                }
                *self.kinds.entry(kind.clone()).or_default() += 1;
                Ok(Fed::Other(kind))
            }
            TableLine::Other { kind } => {
                self.count_kind(line_no, &kind)?;
                Ok(Fed::Other(kind))
            }
            TableLine::Rule(r) => {
                // a streamed rhs: what `parse_line` checks of it, where it checks it (after the fields)
                if streamed.is_some_and(|s| !s.array) && r.format == RecordFormat::TableDump {
                    return Err(self.lerr(
                        line_no,
                        RecordError::Format {
                            line: 1,
                            msg: "table_dump.v1 rule without an rhs array".into(),
                        }
                        .to_string(),
                    ));
                }
                if let Some(e) = streamed.and_then(|s| s.keys_error.as_ref()) {
                    return Err(self.lerr(line_no, e.to_string()));
                }
                if self.header.is_none() {
                    return Err(self.lerr(line_no, "rule record before the header".into()));
                }
                self.refuse_after_end(line_no, "rule")?;
                let mut rec = *r;
                if let Some(s) = streamed
                    && self.opts.rhs == RhsMode::KeysOnly
                {
                    rec.rhs_keys = s.keys.clone();
                }
                rec.line_no = line_no;
                self.rule_lines += 1;
                let this = self.ordinal;
                self.ordinal += 1;
                if !keep(this, rec.index) {
                    return Ok(Fed::RuleNotKept);
                }
                let sha = match sha {
                    Some(s) => s,
                    None => {
                        let mut h = Sha256::new();
                        h.update(body);
                        h.finish_hex()
                    }
                };
                let spans =
                    top_level_spans(body, &["origin", "origin_index", "rhs", "kind", "symface"])
                        .map_err(|m| self.lerr(line_no, m))?;
                let val = |k: &str| -> Result<Option<Value>, ImportError> {
                    match spans.get(k) {
                        None => Ok(None),
                        Some(&(a, b)) => serde_json::from_slice(&body[a..b])
                            .map(Some)
                            .map_err(|e| self.lerr(line_no, format!("field {k}: {e}"))),
                    }
                };
                let domain_bounds = rec.domain_bounds.clone();
                let origin = val("origin")?.and_then(|v| match v {
                    Value::String(s) => Some(s),
                    Value::Null => None,
                    o => Some(o.to_string()),
                });
                let origin_index = val("origin_index")?.and_then(|v| v.as_i64());
                let record_kind = val("kind")?.and_then(|v| v.as_str().map(str::to_string));
                let symface = match val("symface")? {
                    None | Some(Value::Null) => None,
                    Some(v) => Some(SymfaceRef {
                        schema: v.get("schema").and_then(Value::as_str).map(str::to_string),
                        face: v.get("face").cloned().unwrap_or(Value::Null),
                        element: v
                            .get("element")
                            .and_then(|e| e.get("index"))
                            .and_then(Value::as_i64),
                        slice: v.get("slice").cloned().unwrap_or(Value::Null),
                    }),
                };
                let (rhs_span, rhs_sha256) = match streamed {
                    Some(s) => (
                        Some((s.span.0 as usize, s.span.1 as usize)),
                        s.sha256.clone().filter(|_| self.rhs_digest),
                    ),
                    None => {
                        let rhs_span = spans.get("rhs").copied();
                        let rhs_sha256 = rhs_span.filter(|_| self.rhs_digest).map(|(a, b)| {
                            let mut h = Sha256::new();
                            h.update(&body[a..b]);
                            h.finish_hex()
                        });
                        (rhs_span, rhs_sha256)
                    }
                };
                self.n_records += 1;
                Ok(Fed::Rule(Box::new(RawRecord {
                    ordinal: this,
                    line_no,
                    offset,
                    bytes,
                    sha256: sha,
                    rec,
                    domain_bounds,
                    origin,
                    origin_index,
                    rhs_sha256,
                    record_kind,
                    rhs_span,
                    sym: None,
                    symface,
                })))
            }
        }
    }
    /// The scanned table (without records): header required.
    pub fn finish(self, file_sha256: String, file_bytes: u64) -> Result<RawTable, ImportError> {
        let header = self.header.ok_or_else(|| ImportError::Line {
            source: self.source.to_string(),
            path: self.path.display().to_string(),
            line: 0,
            msg: "file without header".into(),
        })?;
        Ok(RawTable {
            header,
            charts: self.charts,
            records: vec![],
            kinds: self.kinds,
            file_sha256,
            file_bytes,
            markers: self.markers,
            rule_lines: self.rule_lines,
            sym_lines: self.sym_lines,
        })
    }
}

/// Per-source policy of the completeness check (interfaces/import.md 18.3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MarkerPolicy {
    /// `marker_rules.accept_fewer`: a marker's `rules` GREATER than the file's rule records is accepted with this
    /// reason (a minimised table that kept the original marker); `None` = strict.
    pub accept_fewer_rules: Option<String>,
    /// `marker_rules.accept_missing_end` (lane partfix4): a file WITHOUT an `end` record is accepted with this reason
    /// (recorded in the overrides); `None` = strict: refused, since a missing `end` cannot be told from a file cut at a
    /// line boundary.
    pub accept_missing_end: Option<String>,
}

/// What a table's markers declare, after [`check_markers`].
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize)]
pub struct MarkerSummary {
    /// Keys of the `composite_terminal_ledger` records: (line, key).
    pub ledger_keys: Vec<(usize, Vec<i32>)>,
    /// Keys of the `residual` records that read as integer keys of the arity: (line, key).
    pub residual_keys: Vec<(usize, Vec<i32>)>,
    /// Residual records + ledger keys (what `residuals` / `expected_LP` must state).
    pub declared_terminals: usize,
    /// The table carries a composite-assembly record (any of the three markers or a ledger).
    pub assembly: bool,
    /// Accepted marker overrides (with the policy's reason).
    pub overrides: Vec<String>,
}

fn as_count(v: &Value) -> Option<u64> {
    v.as_u64()
}

/// Check the markers of a scanned table against the file itself (interfaces/import.md 18.2 / 18.3). `arity`: the
/// frame's (ledger keys must have it). Refusals are typed ([`TableRefusal`]).
pub fn check_markers(
    source: &str,
    path: &Path,
    raw: &RawTable,
    arity: usize,
    policy: &MarkerPolicy,
) -> Result<MarkerSummary, ImportError> {
    let refused = |line: usize, reason: TableRefusal| ImportError::Refused {
        source: source.to_string(),
        path: path.display().to_string(),
        line,
        reason: Box::new(reason),
    };
    let m = &raw.markers;
    let mut out = MarkerSummary {
        assembly: !m.ledgers.is_empty() || !m.completions.is_empty() || !m.provenance.is_empty(),
        ..Default::default()
    };
    for l in &m.ledgers {
        let bad = |msg: String| refused(l.line, TableRefusal::Ledger { msg });
        let keys = match &l.keys {
            Some(Value::Array(a)) => a,
            _ => return Err(bad("no `keys` array".into())),
        };
        for k in keys {
            let t = k
                .as_str()
                .ok_or_else(|| bad(format!("key {k} is not a text")))?;
            let key = parse_key_text(t, arity)
                .ok_or_else(|| bad(format!("key {t:?} is not an integer key of arity {arity}")))?;
            out.ledger_keys.push((l.line, key));
        }
        if let Some(c) = &l.count
            && as_count(c) != Some(keys.len() as u64)
        {
            return Err(bad(format!("count {c} for {} keys", keys.len())));
        }
    }
    for (line, t) in &m.residuals {
        if let Some(k) = parse_key_text(t, arity) {
            out.residual_keys.push((*line, k));
        }
    }
    out.declared_terminals = m.residuals.len() + out.ledger_keys.len();
    for c in &m.completions {
        let mismatch = |field: &str, stated: String, found: String| {
            refused(
                c.line,
                TableRefusal::Completion {
                    marker: c.kind.clone(),
                    field: field.to_string(),
                    stated,
                    found,
                },
            )
        };
        let shown = |v: &Option<Value>| v.as_ref().map_or("absent".to_string(), Value::to_string);
        let required_complete = c.kind == "composite_assembly_complete";
        match &c.complete {
            Some(Value::Bool(true)) => {}
            None if !required_complete => {}
            other => return Err(mismatch("complete", shown(other), "true required".into())),
        }
        let found_rules = raw.rule_lines as u64;
        match c.rules.as_ref().and_then(as_count) {
            Some(n) if n == found_rules => {}
            Some(n) if n > found_rules && policy.accept_fewer_rules.is_some() => {
                out.overrides.push(format!(
                    "{} line {}: rules {n} > {found_rules} rule records accepted: {}",
                    c.kind,
                    c.line,
                    policy.accept_fewer_rules.as_deref().unwrap_or_default()
                ));
            }
            _ => {
                return Err(mismatch(
                    "rules",
                    shown(&c.rules),
                    format!("{found_rules} rule records"),
                ));
            }
        }
        let found_t = out.declared_terminals as u64;
        for (field, v) in [("residuals", &c.residuals), ("expected_LP", &c.expected_lp)] {
            if let Some(x) = v
                && as_count(x) != Some(found_t)
            {
                return Err(mismatch(
                    field,
                    x.to_string(),
                    format!(
                        "{found_t} declared terminals ({} residual records + {} ledger keys)",
                        m.residuals.len(),
                        out.ledger_keys.len()
                    ),
                ));
            }
        }
    }
    let header_order = raw.header.order_name.clone().or_else(|| {
        raw.header
            .raw
            .get("node_order")
            .and_then(|o| o.get("name"))
            .and_then(Value::as_str)
            .map(str::to_string)
    });
    for (line, o) in &m.provenance {
        match o {
            None | Some(Value::Null) => {}
            Some(Value::String(s)) if Some(s) == header_order.as_ref() => {}
            Some(v) => {
                return Err(refused(
                    *line,
                    TableRefusal::OrderMismatch {
                        stated: v.as_str().map_or_else(|| v.to_string(), str::to_string),
                        header: header_order.clone().unwrap_or_else(|| "absent".into()),
                    },
                ));
            }
        }
    }
    // lane partfix4: the end record is the file's statement that it is complete (checked after the markers: a marker
    // that contradicts the file is the more specific refusal)
    if !raw.kinds.contains_key("end") {
        match policy.accept_missing_end.as_deref() {
            Some(why) if !why.trim().is_empty() => out.overrides.push(format!(
                "no end record accepted ({} rule + {} sym records as found): {why}",
                raw.rule_lines, raw.sym_lines
            )),
            _ => {
                return Err(refused(
                    0,
                    TableRefusal::Incomplete {
                        msg: format!(
                            "no end record ({} rule + {} sym records found; a file cut at a line boundary looks the \
                             same; override per source: marker_rules.accept_missing_end with a reason)",
                            raw.rule_lines, raw.sym_lines
                        ),
                    },
                ));
            }
        }
    }
    Ok(out)
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

/// `n0=a,n1=b,..` text (rr_proof's key form) -> key; slots not named are `i32::MIN` (the caller completes them from
/// the record's fixed slots or refuses). `None` on a malformed text, a slot >= arity or a slot named twice.
pub fn parse_named_key_text(t: &str, arity: usize) -> Option<Vec<i32>> {
    let mut k = vec![i32::MIN; arity];
    for part in t.split(',') {
        let (name, v) = part.trim().split_once('=')?;
        let i: usize = name.trim().strip_prefix('n')?.parse().ok()?;
        if i >= arity || k[i] != i32::MIN {
            return None;
        }
        k[i] = v.trim().parse().ok()?;
    }
    Some(k)
}

/// The TERM-KEY DIGEST of a key (interfaces/import.md 5.5): the first 8 bytes (16 lowercase hex characters) of the
/// sha256 of its `I(a,b,..)` text (no spaces). Fact verdict lines list the digests of the NON-ZERO terms only; a term
/// whose digest is not listed has a coefficient identically zero in d at the key and is NOT an edge. A collision can
/// only make a zero term look non-zero (followed: conservative), never the converse.
pub fn term_key_digest(k: &[i32]) -> u64 {
    let mut h = Sha256::new();
    h.update(key_text(k).as_bytes());
    let hex = h.finish_hex();
    u64::from_str_radix(&hex[..16], 16).unwrap_or(0)
}

/// Parse a 16-hex term-key digest.
pub fn parse_term_key_digest(s: &str) -> Option<u64> {
    (s.len() == 16)
        .then(|| u64::from_str_radix(s, 16).ok())
        .flatten()
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
