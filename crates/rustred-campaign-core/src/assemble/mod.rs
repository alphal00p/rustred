//! TABLE ASSEMBLY (interface `interfaces/acceptance.md` section 5; plan 2.4.7 D1/D6/D7, 2.8, R20, R33).
//!
//! From a node's input tables, the ACCEPTED campaign rules ([`Accepted`] tokens of the acceptance chain: nothing else
//! is assembled), symmetry credits, declared terminals, retirements and removals: ONE dispatch order (digest), ONE
//! `table_dump.v1` node table with ONE header that the engine's `Table::load` reads (rule records in dispatch order =
//! `find_rule` order, terminals as `residual` records), a side manifest naming the source of every record, and
//! `assembly.extras.jsonl` for what the engine cannot express today (exclusions that are not made safe in the table,
//! symmetry credits). Rule records are BYTE COPIES of the source lines (only the top-level `index` is renumbered),
//! streamed one at a time (records can be 100+ MB; the coefficient text is never re-serialised).
//!
//! Checks (audit-accept-A / -B): every input line is classified (an unknown or unreadable line is an error, never
//! dropped); every input carries a header and all headers (inputs and campaign sources) agree on every identity field,
//! the chart, symmetry-group and zero-sector lines; every token was judged for this node, target, frame and comparator;
//! the SET of records placed before each campaign rule is the set it was judged against; exclusions come from the
//! tokens and are made safe on the FINAL order; FULL-node retirements are refused unless the partition proofs are
//! certified and the proof binds this assembly's tables; FULL-node removals are refused.

pub mod domaincheck;
pub mod recipes;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::accept::{
    Accepted, Exclusion, ExclusionTarget, NodeTarget, Position, PriorityClass, Scope, SetDigest,
    node_identity,
};
use crate::closure::sha256::Sha256;
use crate::cover::{Dispatcher, Table};
use crate::domain::{CaseDomain, DomainOptions, Stage};
use crate::economy::{CoverageState, RetireError, Retirement};
use crate::key::{Key, NodeFrame};
use crate::order::NodeComparator;
use crate::partition::{DispatchOrderStatement, DispatcherSource, parse_dispatch_proof_method};
use crate::record::{
    ReadOptions, RecordReader, RhsMode, RuleRecord, TableFile, TableHeader, TableLine,
};

pub const ASSEMBLY_SCHEMA: &str = "rustred.campaign.assembly.v1";
pub const MANIFEST_SCHEMA: &str = "rustred.campaign.assembly-manifest.v1";
pub const EXTRAS_SCHEMA: &str = "rustred.campaign.assembly-extras.v1";
pub const ORDER_SCHEMA: &str = "rustred.campaign.dispatch-order.v1";

/// Header fields that describe how a table was PRODUCED and may differ between tables of one node; every other
/// header field is part of the table's IDENTITY and must agree across all inputs and campaign sources
/// (audit-accept-A C2, B C5). A field missing from this list is compared (fail closed).
pub const HEADER_PROVENANCE_FIELDS: &[&str] = &[
    "source_base",
    "family_file",
    "zero_seconds",
    "node_options",
    "numerical_depth",
    "max_numerator_rank",
    "source_conditions",
    "assembly",
    "seconds",
    "harness",
    "host",
    "created",
];

/// Line kinds of a `table_dump.v1` file and what assembly does with them. Any other kind is an error.
const COPIED_ONCE: &[&str] = &["zero_sectors", "chart", "symmetry_group"];
const NOT_COPIED: &[&str] = &["residual", "chart_end", "end", "composite_terminal_ledger"];

/// An input table of the node with its spec priority (D1: fixed in the spec at campaign start).
#[derive(Clone, Debug)]
pub struct InputTable {
    pub id: String,
    pub path: PathBuf,
    pub priority: u32,
    /// The expected `sha256:` of the file (audit-accept-A M8): verified when given.
    pub sha256: Option<String>,
}

/// What assembly does with a FULL-node retirement (audit-accept-A M2, B M3; the partition proofs are under repair,
/// audit-part-A): `Refuse` (default) refuses every FULL retirement; `BoundProof` (to be set ONLY once the partition
/// fix is merged and announced) accepts one whose containment proof binds THIS assembly's table sets in force before
/// and after the retirement ([`tables_in_force`], digested by the one definition
/// [`crate::economy::table_set_digest`] and checked with [`Retirement::check_tables`]), states the same dispatch order,
/// whose node is this node and whose `by` rule stays in the assembled set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub enum FullRetirements {
    #[default]
    Refuse,
    BoundProof,
}

/// A symmetry credit (plan 2.4.6) as the closure records it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CreditRec {
    pub key: Key,
    pub element: usize,
    pub image: Key,
    /// `sha256:` of the serving rule's record line.
    pub rule_sha256: String,
}

/// A D7 (i) removal: the rule was disproved at a served key. Refused on a FULL node (coverage would shrink silently,
/// audit-accept-B M3); on a BOXED node the rules after it are re-checked by the order check (they must have been
/// re-judged without it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Removal {
    pub rule_sha256: String,
    pub reason: String,
    pub disproof: String,
}

/// What to assemble.
pub struct AssemblyInput {
    pub node: String,
    pub target: NodeTarget,
    pub inputs: Vec<InputTable>,
    /// The accepted campaign rules: tokens of the acceptance chain only (audit-accept-A M1, B M2).
    pub accepted: Vec<Accepted>,
    pub terminals: Vec<Key>,
    pub credits: Vec<CreditRec>,
    /// Retirements (economy, R33: a FULL node's retirement carries a containment proof by construction).
    pub retirements: Vec<Retirement>,
    pub removals: Vec<Removal>,
    pub full_retirements: FullRetirements,
    /// Copy records whose `dispatch` is not Partition (both matchers skip them).
    pub keep_skipped: bool,
    /// Rules that win at least one key of the final CLOSED closure (line sha256): BOXED nodes ship only these (R20);
    /// FULL nodes keep everything (R33). `None` = no report attached.
    pub used: Option<BTreeSet<String>>,
    /// Rewrite the top-level `index` of every record to its assembled position (the ONLY byte change; unique
    /// indices = `find_rule`'s rule number, so `rr_proof --rule`, closure labels and domaincheck agree). The manifest
    /// keeps the source index and the source line digest.
    pub renumber_index: bool,
    /// Number of recipe records shipped beside the table (`assemble::recipes`): > 0 makes the set not engine-loadable
    /// today (interface 10.4, audit-accept-A M3).
    pub recipes: usize,
}

/// Errors (I/O and inconsistent inputs; never a silent pass).
#[derive(Debug)]
pub enum AssembleError {
    Io(std::io::Error),
    HeaderMismatch {
        field: String,
        table: String,
    },
    Record {
        source: String,
        detail: String,
    },
    Retire(String),
    /// A campaign rule's judged tables in force differ from the records placed before it (audit-accept-B C7): re-judge
    /// it against the assembled prefix (plan D6).
    Order {
        rule: String,
        detail: String,
    },
    /// A token judged for another node / target / frame / comparator.
    Binding {
        rule: String,
        detail: String,
    },
    Other(String),
}

impl std::fmt::Display for AssembleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AssembleError::Io(e) => write!(f, "io: {e}"),
            AssembleError::HeaderMismatch { field, table } => {
                write!(f, "header field {field} of {table} differs")
            }
            AssembleError::Record { source, detail } => write!(f, "{source}: {detail}"),
            AssembleError::Retire(e) => write!(f, "retirement: {e}"),
            AssembleError::Order { rule, detail } => {
                write!(f, "dispatch order at {rule}: {detail}")
            }
            AssembleError::Binding { rule, detail } => write!(f, "token of {rule}: {detail}"),
            AssembleError::Other(e) => write!(f, "{e}"),
        }
    }
}
impl std::error::Error for AssembleError {}
impl From<std::io::Error> for AssembleError {
    fn from(e: std::io::Error) -> Self {
        AssembleError::Io(e)
    }
}

/// One rule record located in a source file.
#[derive(Clone, Debug)]
struct Entry {
    source_id: String,
    path: PathBuf,
    source_sha256: String,
    ordinal: u64,
    offset: u64,
    bytes: u64,
    line_sha256: String,
    record: RuleRecord,
    position: Position,
    campaign: Option<usize>,
    /// dispatched (Partition) record
    dispatched: bool,
    hoisted_for: Vec<Key>,
}

/// The dispatch order (D1): input tables with priorities and digests, then the campaign rules with positions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DispatchOrder {
    pub schema: &'static str,
    pub node: String,
    pub tables: Vec<OrderTable>,
    pub campaign_rules: Vec<OrderRule>,
    pub terminals: usize,
    pub credits: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrderTable {
    pub id: String,
    pub priority: u32,
    pub sha256: String,
    pub records: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct OrderRule {
    pub line_sha256: String,
    pub position: Position,
    /// 0-based position of the record in the assembled table.
    pub n: u64,
    /// The acceptance record id of the token.
    pub acceptance_record: String,
}

/// An exclusion the assembled table does not make safe (export blocked).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Blocking {
    pub rule_n: u64,
    pub rule_sha256: String,
    pub target: ExclusionTarget,
    pub why: String,
}

/// What the assembly did.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct AssemblyReport {
    pub records: u64,
    pub input_records: u64,
    pub campaign_records: u64,
    pub skipped_not_partition: u64,
    pub retired: Vec<String>,
    pub removed: Vec<String>,
    pub dropped_unused: Vec<String>,
    /// (assembled position of the hoisted point record, keys it now serves before the excluding rule)
    pub hoisted: Vec<(u64, Vec<Key>)>,
    pub exclusions_safe: u64,
    pub blocking: Vec<Blocking>,
    pub credits_need_importer: u64,
    /// Campaign rules whose judged in-force set equals the assembled prefix (all of them, else the assembly fails).
    pub order_checked: u64,
    /// `residual` records of the input tables whose key is not a declared terminal (not copied: the shipped table
    /// would lose these masters; export blocked).
    pub undeclared_input_terminals: Vec<Key>,
    /// Why the set is not export-ready (empty iff `export_ready`).
    pub not_ready: Vec<String>,
    /// Ready for the engine, modulo `dispatch_equal` (see [`AssemblyOutput::export_ready_with`]).
    pub export_ready: bool,
}

/// Files written and digests.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AssemblyOutput {
    pub table: PathBuf,
    pub table_sha256: String,
    pub manifest: PathBuf,
    pub manifest_sha256: String,
    pub extras: PathBuf,
    pub order: DispatchOrder,
    pub order_digest: String,
    pub report: AssemblyReport,
}

impl AssemblyOutput {
    /// Export-ready AND `dispatch_equal` passed on a non-empty key set (audit-accept-A M3, m4): the core dispatcher and
    /// the engine's matcher agree on the assembled file, with `core_blocked` keys allowed only when declared.
    pub fn export_ready_with(
        &self,
        eq: &domaincheck::DispatchEqual,
        allow_core_blocked: bool,
    ) -> bool {
        self.report.export_ready && eq.equal(allow_core_blocked)
    }
}

fn sha_tag(h: Sha256) -> String {
    format!("sha256:{}", h.finish_hex())
}

fn parse_rule_line(line: &[u8], source: &str) -> Result<RuleRecord, AssembleError> {
    match RecordReader::new(line, ReadOptions { rhs: RhsMode::Skip }).next() {
        Some(Ok(TableLine::Rule(r))) => Ok(*r),
        Some(Ok(_)) => Err(AssembleError::Record {
            source: source.into(),
            detail: "not a rule line".into(),
        }),
        Some(Err(e)) => Err(AssembleError::Record {
            source: source.into(),
            detail: e.to_string(),
        }),
        None => Err(AssembleError::Record {
            source: source.into(),
            detail: "empty".into(),
        }),
    }
}

/// The `"type"` of a table line (audit-accept-B C4: never guessed from a prefix that may not hold it). Fast path: a line
/// that starts with `{"type":"<kind>"` exactly as the engine writes it; otherwise the WHOLE line is scanned for its
/// top-level `type` (nothing else is kept, so a 100 MB record costs one pass). `Err` = not a JSON object; `Ok(None)` =
/// an object without a string `type`.
pub(crate) fn line_kind(line: &[u8]) -> Result<Option<String>, String> {
    if let Some(rest) = line.strip_prefix(br#"{"type":""#)
        && let Some(end) = rest.iter().take(64).position(|&b| b == b'"')
        && rest[..end]
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || *b == b'_')
    {
        return Ok(Some(String::from_utf8_lossy(&rest[..end]).into_owned()));
    }
    #[derive(serde::Deserialize)]
    struct Kind<'a> {
        #[serde(rename = "type", borrow)]
        t: Option<std::borrow::Cow<'a, str>>,
    }
    serde_json::from_slice::<Kind>(line)
        .map(|k| k.t.map(|t| t.into_owned()))
        .map_err(|e| e.to_string())
}

/// Scan of one input table: header value (and line), copied-once lines, rule entries, input terminals.
struct Scanned {
    header: Option<Value>,
    header_line: Option<Vec<u8>>,
    zero_sectors: Option<Value>,
    charts: Vec<Value>,
    symmetry: Vec<Value>,
    /// raw bytes of the copied-once lines in file order (written from the first table)
    once_lines: Vec<Vec<u8>>,
    entries: Vec<Entry>,
    residual_keys: Vec<Key>,
    sha256: String,
    skipped: u64,
}

fn scan_input(t: &InputTable, keep_skipped: bool) -> Result<Scanned, AssembleError> {
    let mut rd = BufReader::with_capacity(1 << 20, File::open(&t.path)?);
    let mut whole = Sha256::new();
    let mut buf = Vec::new();
    let mut off = 0u64;
    let mut out = Scanned {
        header: None,
        header_line: None,
        zero_sectors: None,
        charts: vec![],
        symmetry: vec![],
        once_lines: vec![],
        entries: vec![],
        residual_keys: vec![],
        sha256: String::new(),
        skipped: 0,
    };
    let bad = |line_no: usize, d: String| AssembleError::Record {
        source: t.id.clone(),
        detail: format!("line {line_no}: {d}"),
    };
    let mut ordinal = 0u64;
    let mut line_no = 0usize;
    let mut last_kind = String::new();
    let mut ends = 0usize;
    loop {
        buf.clear();
        let n = rd.read_until(b'\n', &mut buf)? as u64;
        if n == 0 {
            break;
        }
        line_no += 1;
        whole.update(&buf);
        let start = off;
        off += n;
        let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let kind = line_kind(line)
            .map_err(|e| bad(line_no, format!("not a JSON object: {e}")))?
            .ok_or_else(|| bad(line_no, "a line without a \"type\"".into()))?;
        if ends > 0 {
            return Err(bad(line_no, format!("a {kind} line after the end record")));
        }
        let parse = |line: &[u8]| -> Result<Value, AssembleError> {
            serde_json::from_slice(line).map_err(|e| bad(line_no, e.to_string()))
        };
        match kind.as_str() {
            "rule" => {
                let rec = parse_rule_line(line, &t.id)?;
                let skip = rec.dispatch.as_deref().is_some_and(|d| d != "Partition");
                let this = ordinal;
                ordinal += 1;
                if skip {
                    out.skipped += 1;
                    if !keep_skipped {
                        continue;
                    }
                }
                let mut h = Sha256::new();
                h.update(line);
                out.entries.push(Entry {
                    source_id: t.id.clone(),
                    path: t.path.clone(),
                    source_sha256: String::new(),
                    ordinal: this,
                    offset: start,
                    bytes: line.len() as u64,
                    line_sha256: sha_tag(h),
                    record: rec,
                    position: Position {
                        class: PriorityClass::Input,
                        input_priority: t.priority,
                        round: 0,
                        item: t.id.clone(),
                        index: this as u32,
                    },
                    campaign: None,
                    dispatched: !skip,
                    hoisted_for: vec![],
                });
            }
            "header" => {
                if out.header.is_some() {
                    return Err(bad(line_no, "second header".into()));
                }
                if out.entries.len() + out.once_lines.len() > 0 {
                    return Err(bad(line_no, "header after other lines".into()));
                }
                out.header = Some(parse(line)?);
                out.header_line = Some(line.to_vec());
            }
            "zero_sectors" => {
                if out.zero_sectors.is_some() {
                    return Err(bad(line_no, "second zero_sectors line".into()));
                }
                out.zero_sectors = Some(parse(line)?);
                out.once_lines.push(line.to_vec());
            }
            "chart" => {
                out.charts.push(parse(line)?);
                out.once_lines.push(line.to_vec());
            }
            "symmetry_group" => {
                out.symmetry.push(parse(line)?);
                out.once_lines.push(line.to_vec());
            }
            "residual" => {
                let v = parse(line)?;
                let k = v
                    .get("key")
                    .and_then(Value::as_str)
                    .and_then(parse_key_text)
                    .ok_or_else(|| bad(line_no, "residual without a readable key".into()))?;
                out.residual_keys.push(k);
            }
            "end" => ends += 1,
            k if NOT_COPIED.contains(&k) || COPIED_ONCE.contains(&k) => {}
            other => {
                return Err(bad(
                    line_no,
                    format!(
                        "unknown line kind {other:?} (assembly copies or drops only known kinds)"
                    ),
                ));
            }
        }
        last_kind = kind;
    }
    if out.header.is_none() {
        return Err(AssembleError::Record {
            source: t.id.clone(),
            detail: "input table without a header (refused: its frame and order are unknown)"
                .into(),
        });
    }
    if ends != 1 || last_kind != "end" {
        return Err(AssembleError::Record {
            source: t.id.clone(),
            detail: "no end record as the last line (truncated table?)".into(),
        });
    }
    out.sha256 = sha_tag(whole);
    if let Some(want) = &t.sha256
        && *want != out.sha256
        && format!("sha256:{want}") != out.sha256
    {
        return Err(AssembleError::Record {
            source: t.id.clone(),
            detail: format!("file digest {}, expected {want}", out.sha256),
        });
    }
    for e in &mut out.entries {
        e.source_sha256 = out.sha256.clone();
    }
    Ok(out)
}

/// The header of a campaign source file, if its first non-blank line is one.
fn source_header(path: &Path) -> Result<Option<Value>, AssembleError> {
    let mut rd = BufReader::with_capacity(1 << 16, File::open(path)?);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if rd.read_until(b'\n', &mut buf)? == 0 {
            return Ok(None);
        }
        let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        return match line_kind(line) {
            Ok(Some(k)) if k == "header" => {
                serde_json::from_slice(line)
                    .map(Some)
                    .map_err(|e| AssembleError::Record {
                        source: path.display().to_string(),
                        detail: e.to_string(),
                    })
            }
            _ => Ok(None),
        };
    }
}

/// The identity part of a header: every field except [`HEADER_PROVENANCE_FIELDS`].
pub fn header_identity(h: &Value) -> BTreeMap<String, Value> {
    h.as_object()
        .map(|m| {
            m.iter()
                .filter(|(k, _)| !HEADER_PROVENANCE_FIELDS.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// The first identity field where two headers differ.
fn header_diff(a: &Value, b: &Value) -> Option<String> {
    let (ia, ib) = (header_identity(a), header_identity(b));
    ia.keys()
        .chain(ib.keys())
        .find(|k| ia.get(*k) != ib.get(*k))
        .cloned()
}

fn read_line_at(path: &Path, offset: u64) -> Result<Vec<u8>, AssembleError> {
    let mut f = File::open(path)?;
    f.seek(SeekFrom::Start(offset))?;
    let mut rd = BufReader::with_capacity(1 << 20, f);
    let mut buf = Vec::new();
    rd.read_until(b'\n', &mut buf)?;
    if buf.last() == Some(&b'\n') {
        buf.pop();
    }
    Ok(buf)
}

/// Outcome of a top-level `index` splice on a record head.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Splice {
    /// The head with the index replaced.
    Done(Vec<u8>),
    /// No top-level `"index"` key in this head (read more of the line, or the record has none).
    NotInHead,
    /// Refused: the value differs from the expected one, is not an integer, ends at the head's end, or the key repeats.
    Refused(String),
}

/// Replace the TOP-LEVEL `"index":<int>` of a record head by `n` (string-aware scan, depth 1 only; a string VALUE
/// `"index"` is skipped, audit-accept-B m3); the value found must equal `expected`; a second top-level `"index"` key
/// in the head is refused.
pub fn splice_index_head(head: &[u8], expected: Option<i64>, n: u64) -> Splice {
    let pat = br#""index""#;
    let mut depth = 0i32;
    let mut i = 0usize;
    let mut found: Option<(usize, usize)> = None;
    while i < head.len() {
        match head[i] {
            b'"' => {
                if depth == 1 && head[i..].starts_with(pat) {
                    let mut j = i + pat.len();
                    while j < head.len() && head[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if head.get(j) == Some(&b':') {
                        // a key
                        if found.is_some() {
                            return Splice::Refused("a second top-level \"index\" key".into());
                        }
                        j += 1;
                        while j < head.len() && head[j].is_ascii_whitespace() {
                            j += 1;
                        }
                        let start = j;
                        if head.get(j) == Some(&b'-') {
                            j += 1;
                        }
                        while j < head.len() && head[j].is_ascii_digit() {
                            j += 1;
                        }
                        if j >= head.len() {
                            return Splice::NotInHead;
                        }
                        let v: Option<i64> = std::str::from_utf8(&head[start..j])
                            .ok()
                            .and_then(|t| t.parse().ok());
                        if v.is_none() || v != expected {
                            return Splice::Refused(format!(
                                "top-level index {v:?}, the record says {expected:?}"
                            ));
                        }
                        found = Some((start, j));
                        i = j;
                        continue;
                    }
                }
                // skip the string (key or value)
                i += 1;
                while i < head.len() && head[i] != b'"' {
                    if head[i] == b'\\' {
                        i += 1;
                    }
                    i += 1;
                }
            }
            b'{' | b'[' => depth += 1,
            b'}' | b']' => depth -= 1,
            _ => {}
        }
        i += 1;
    }
    match found {
        Some((start, j)) => {
            let mut out = Vec::with_capacity(head.len() + 8);
            out.extend_from_slice(&head[..start]);
            out.extend_from_slice(n.to_string().as_bytes());
            out.extend_from_slice(&head[j..]);
            Splice::Done(out)
        }
        None => Splice::NotInHead,
    }
}

/// [`splice_index_head`] as an `Option` (compatibility): `None` unless the splice was done.
pub fn splice_index(head: &[u8], expected: Option<i64>, n: u64) -> Option<Vec<u8>> {
    match splice_index_head(head, expected, n) {
        Splice::Done(v) => Some(v),
        _ => None,
    }
}

/// Copy one record line from `f` (positioned at its start, `bytes` long) to `w` with its top-level index replaced by
/// `n` (when `renumber`): the head grows until the index is found (records whose index lies beyond the first MiB are
/// no longer refused, audit-accept-A m5 / B m3). Returns the sha256 of the SOURCE bytes.
fn copy_record(
    f: &mut File,
    bytes: u64,
    renumber: Option<(Option<i64>, u64)>,
    put: &mut dyn FnMut(&[u8]) -> std::io::Result<()>,
    source: &str,
) -> Result<String, AssembleError> {
    let mut left = bytes;
    let mut lh = Sha256::new();
    if let Some((expected, n)) = renumber {
        let mut head: Vec<u8> = Vec::new();
        let mut want = (left as usize).min(1 << 20);
        loop {
            let have = head.len();
            head.resize(want, 0);
            f.read_exact(&mut head[have..])?;
            match splice_index_head(&head, expected, n) {
                Splice::Done(out) => {
                    lh.update(&head);
                    put(&out)?;
                    left -= head.len() as u64;
                    break;
                }
                Splice::NotInHead if (head.len() as u64) < bytes => {
                    want = ((head.len() * 2) as u64).min(bytes) as usize;
                }
                Splice::NotInHead => {
                    return Err(AssembleError::Record {
                        source: source.into(),
                        detail: "no top-level \"index\" field in the record (renumbering refused)"
                            .into(),
                    });
                }
                Splice::Refused(why) => {
                    return Err(AssembleError::Record {
                        source: source.into(),
                        detail: format!("renumbering refused: {why}"),
                    });
                }
            }
        }
    }
    let mut chunk = vec![0u8; 1 << 20];
    while left > 0 {
        let m = (left as usize).min(chunk.len());
        f.read_exact(&mut chunk[..m])?;
        lh.update(&chunk[..m]);
        put(&chunk[..m])?;
        left -= m as u64;
    }
    Ok(sha_tag(lh))
}

/// A key as the engine's `I(..)` text.
pub fn key_text(k: &[i32]) -> String {
    format!(
        "I({})",
        k.iter().map(i32::to_string).collect::<Vec<_>>().join(",")
    )
}

/// Parse `I(a,b,..)` or `a,b,..`.
pub fn parse_key_text(s: &str) -> Option<Key> {
    let s = s.trim();
    let inner = s
        .strip_prefix("I(")
        .and_then(|x| x.strip_suffix(')'))
        .unwrap_or(s);
    inner
        .split(',')
        .map(|x| x.trim().parse::<i32>().ok())
        .collect()
}

// (re-audit ladder-A M3, lane ladderfix2: the former `containment_proof(report, ..)` minted a FULL-node proof from a
// caller-supplied partition report, i.e. from a verdict; removed. The only constructor is now
// `crate::economy::prove_coverage_preserved`, which runs the dispatch-mode partition itself.)

fn domain_of(rec: &RuleRecord, frame: &NodeFrame) -> Option<CaseDomain> {
    CaseDomain::from_rule_record(rec, frame, DomainOptions::default()).ok()
}

fn is_point(rec: &RuleRecord) -> bool {
    !rec.fixed.is_empty() && rec.fixed.iter().all(Option::is_some)
}

/// The assembled header line: the FIRST input header's bytes VERBATIM with the `assembly` provenance object appended
/// before its closing brace (audit-accept-A n2), so the line still starts with `{"type":"header"` (the assembled file
/// is itself a valid input, audit-accept-B N5). A header that already carries `assembly` (re-assembly) is re-written
/// with `type` first and the old provenance replaced.
fn header_line(line: &[u8], value: &Value, assembly: &Value) -> Result<Vec<u8>, AssembleError> {
    let trimmed = line.trim_ascii_end();
    if value.get("assembly").is_none() && trimmed.last() == Some(&b'}') && trimmed.len() > 2 {
        let mut out = trimmed[..trimmed.len() - 1].to_vec();
        out.extend_from_slice(b",\"assembly\":");
        out.extend_from_slice(assembly.to_string().as_bytes());
        out.push(b'}');
        return Ok(out);
    }
    let mut m = value
        .as_object()
        .cloned()
        .ok_or_else(|| AssembleError::Other("header is not an object".into()))?;
    m.remove("type");
    m.insert("assembly".into(), assembly.clone());
    let rest = serde_json::to_string(&Value::Object(m))
        .map_err(|e| AssembleError::Other(e.to_string()))?;
    Ok(format!("{{\"type\":\"header\",{}", &rest[1..]).into_bytes())
}

/// What [`collect`] gathers before retirements: the scanned inputs, the identity checks done, every record (inputs
/// and accepted campaign rules) located and sorted by dispatch position.
struct Collected {
    report: AssemblyReport,
    scans: Vec<Scanned>,
    header: Value,
    frame: NodeFrame,
    entries: Vec<Entry>,
    order_tables: Vec<OrderTable>,
}

/// The first phase of [`assemble`] (shared with [`tables_in_force`]): inputs by spec priority, header identity,
/// campaign tokens bound to this node / target / frame + comparator, duplicates refused, records in dispatch order.
fn collect(input: &AssemblyInput) -> Result<Collected, AssembleError> {
    let mut report = AssemblyReport::default();
    // ---- inputs (sorted by spec priority, then id); every input needs a header and an end record
    let mut inputs = input.inputs.clone();
    inputs.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    if inputs.is_empty() {
        return Err(AssembleError::Other("no input table".into()));
    }
    let mut scans = Vec::new();
    for t in &inputs {
        scans.push(scan_input(t, input.keep_skipped)?);
    }
    let header = scans[0]
        .header
        .clone()
        .expect("scan_input requires a header");
    // ---- header identity (audit-accept-A C2, B C5): every identity field, chart / symmetry / zero-sector lines
    let mut charts0 = scans[0].charts.clone();
    charts0.sort_by_key(|v| v.to_string());
    let mut sym0 = scans[0].symmetry.clone();
    sym0.sort_by_key(|v| v.to_string());
    for (s, t) in scans.iter().zip(&inputs).skip(1) {
        let h = s.header.as_ref().expect("checked");
        if let Some(f) = header_diff(h, &header) {
            return Err(AssembleError::HeaderMismatch {
                field: f,
                table: t.id.clone(),
            });
        }
        if s.zero_sectors != scans[0].zero_sectors {
            return Err(AssembleError::HeaderMismatch {
                field: "zero_sectors line".into(),
                table: t.id.clone(),
            });
        }
        let mut c = s.charts.clone();
        c.sort_by_key(|v| v.to_string());
        if c != charts0 {
            return Err(AssembleError::HeaderMismatch {
                field: "chart lines".into(),
                table: t.id.clone(),
            });
        }
        let mut y = s.symmetry.clone();
        y.sort_by_key(|v| v.to_string());
        if y != sym0 {
            return Err(AssembleError::HeaderMismatch {
                field: "symmetry_group lines".into(),
                table: t.id.clone(),
            });
        }
    }
    let th = TableHeader::from_value(header.clone()).map_err(AssembleError::Other)?;
    let frame = NodeFrame::from_header(&th).map_err(|e| AssembleError::Other(e.to_string()))?;
    let cmp =
        NodeComparator::from_header(&header).map_err(|e| AssembleError::Other(format!("{e:?}")))?;
    let identity = node_identity(&frame, &cmp);
    let declared: BTreeSet<&Key> = input.terminals.iter().collect();
    let mut entries: Vec<Entry> = Vec::new();
    let mut order_tables = Vec::new();
    for (s, t) in scans.iter().zip(&inputs) {
        report.skipped_not_partition += s.skipped;
        order_tables.push(OrderTable {
            id: t.id.clone(),
            priority: t.priority,
            sha256: s.sha256.clone(),
            records: s.entries.len() as u64,
        });
        entries.extend(s.entries.iter().cloned());
        for k in &s.residual_keys {
            if !declared.contains(k) && !report.undeclared_input_terminals.contains(k) {
                report.undeclared_input_terminals.push(k.clone());
            }
        }
    }
    // ---- campaign rules: tokens only, bound to this node / target / frame + comparator
    let mut checked_sources: BTreeSet<PathBuf> = BTreeSet::new();
    for (ci, a) in input.accepted.iter().enumerate() {
        let name = a.rule_sha256().to_string();
        let bind = |d: String| AssembleError::Binding {
            rule: name.clone(),
            detail: d,
        };
        if a.node() != input.node {
            return Err(bind(format!("judged for node {}", a.node())));
        }
        if *a.target() != input.target {
            return Err(bind(format!("judged for target {:?}", a.target())));
        }
        if a.node_identity() != identity {
            return Err(bind(
                "judged in another frame / comparator than the assembled header's".into(),
            ));
        }
        if a.position().class == PriorityClass::Input {
            return Err(bind("a campaign rule in the input class".into()));
        }
        let src = a.source();
        let path = PathBuf::from(&src.table_path);
        let offset = src
            .byte_offset
            .ok_or_else(|| bind("the token's source has no byte offset".into()))?;
        if checked_sources.insert(path.clone())
            && let Some(h) = source_header(&path)?
            && let Some(f) = header_diff(&h, &header)
        {
            return Err(AssembleError::HeaderMismatch {
                field: f,
                table: src.table_id.clone(),
            });
        }
        let line = read_line_at(&path, offset)?;
        let mut h = Sha256::new();
        h.update(&line);
        let d = sha_tag(h);
        if d != name {
            return Err(AssembleError::Record {
                source: src.table_id.clone(),
                detail: format!("line at offset {offset} has digest {d}, expected {name}"),
            });
        }
        let rec = parse_rule_line(&line, &src.table_id)?;
        entries.push(Entry {
            source_id: src.table_id.clone(),
            path,
            source_sha256: src.table_digest.clone(),
            ordinal: src.ordinal,
            offset,
            bytes: line.len() as u64,
            line_sha256: d,
            record: rec,
            position: a.position().clone(),
            campaign: Some(ci),
            dispatched: true,
            hoisted_for: vec![],
        });
    }
    // ---- duplicates (audit-accept-A m3): the same record twice would get one assembled index
    {
        let mut at: BTreeSet<(PathBuf, u64)> = BTreeSet::new();
        let mut digests: BTreeSet<&str> = BTreeSet::new();
        for e in &entries {
            if !at.insert((e.path.clone(), e.offset)) || !digests.insert(&e.line_sha256) {
                return Err(AssembleError::Record {
                    source: e.source_id.clone(),
                    detail: format!("record {} given twice", e.line_sha256),
                });
            }
        }
    }
    entries.sort_by(|a, b| a.position.cmp(&b.position));
    Ok(Collected {
        report,
        scans,
        header,
        frame,
        entries,
        order_tables,
    })
}

/// The DISPATCHED records of `records` (in the given order) as the core dispatcher loads them: one table built by
/// [`Table::from_table_file`] from the assembled header and chart lines (domains, witness status and its block exactly
/// as for any `table_dump.v1` file; records whose `dispatch` is not Partition are skipped by the loader). Labels are
/// the positions in `records` (input indices may repeat across sources; labels enter no digest).
fn dispatch_set<'a>(
    records: impl Iterator<Item = &'a RuleRecord>,
    header: &Value,
    charts: &[Value],
) -> Result<Dispatcher, AssembleError> {
    let th = TableHeader::from_value(header.clone()).map_err(AssembleError::Other)?;
    let mut tf = TableFile {
        header: Some(th),
        ..Default::default()
    };
    for c in charts {
        let chart = c
            .get("chart")
            .and_then(Value::as_str)
            .ok_or_else(|| AssembleError::Other("chart line without chart".into()))?
            .to_string();
        let guards = match c.get("guards") {
            None | Some(Value::Null) => vec![],
            Some(g) => serde_json::from_value::<Vec<String>>(g.clone())
                .map_err(|e| AssembleError::Other(format!("chart guards: {e}")))?,
        };
        tf.charts.push((chart, guards));
    }
    for (n, r) in records.enumerate() {
        let mut r = r.clone();
        r.i = None;
        r.index = Some(n as i64);
        tf.rules.push(r);
    }
    let table =
        Table::from_table_file("tables_in_force", "assembly", &tf, DomainOptions::default())
            .map_err(|e| AssembleError::Retire(format!("tables in force: {e}")))?;
    Dispatcher::new(vec![table], vec![0])
        .map_err(|e| AssembleError::Retire(format!("tables in force: {e:?}")))
}

/// The table set this assembly has IN FORCE before the next FULL-node retirement (int4): the dispatched records of
/// the inputs and accepted campaign rules in dispatch order, with the retirements already in `input.retirements`
/// applied in order, minus the records `minus` (line `sha256:` digests). A FULL-node retirement appended to
/// `input.retirements` is accepted (with [`FullRetirements::BoundProof`]) only if its containment proof was made by
/// [`crate::economy::prove_coverage_preserved`] with `old` = `tables_in_force(input, &[])` and `new` =
/// `tables_in_force(input, retired)` (as [`DispatcherSource`]s), order statement `SameOrder`: assembly recomputes both
/// sets and checks the proof with [`Retirement::check_tables`]. The caller proves on exactly what assembly checks.
pub fn tables_in_force(
    input: &AssemblyInput,
    minus: &[String],
) -> Result<Dispatcher, AssembleError> {
    let c = collect(input)?;
    let mut entries = c.entries;
    for r in &input.retirements {
        entries.retain(|e| !r.retired().contains(&e.line_sha256));
    }
    entries.retain(|e| !minus.contains(&e.line_sha256));
    dispatch_set(
        entries.iter().map(|e| &e.record),
        &c.header,
        &c.scans[0].charts,
    )
}

/// Assemble into `out_dir` (files `<node>.assembled.table.jsonl`, `<node>.assembly.manifest.jsonl`,
/// `<node>.assembly.extras.jsonl`, `<node>.dispatch-order.json`).
pub fn assemble(input: &AssemblyInput, out_dir: &Path) -> Result<AssemblyOutput, AssembleError> {
    let full = input.target == NodeTarget::Full;
    let Collected {
        mut report,
        scans,
        header,
        frame,
        mut entries,
        order_tables,
    } = collect(input)?;
    // ---- retirements and removals (identity = line sha256)
    if full && !input.removals.is_empty() {
        return Err(AssembleError::Retire(
            "a removal on a FULL node shrinks coverage without a proof (refused)".into(),
        ));
    }
    for r in &input.retirements {
        if full {
            if input.full_retirements == FullRetirements::Refuse {
                return Err(AssembleError::Retire(
                    "FULL-node retirements are refused until the partition proofs are certified \
                     (FullRetirements::BoundProof)"
                        .into(),
                ));
            }
            let proof = match (r.state(), r.proof()) {
                (CoverageState::Full, Some(p)) => p,
                _ => {
                    return Err(AssembleError::Retire(format!(
                        "{:?} on a FULL node needs a FULL retirement with a containment proof",
                        RetireError::FullNodeNeedsProof
                    )));
                }
            };
            if r.node() != input.node {
                return Err(AssembleError::Retire(format!(
                    "retirement of node {}",
                    r.node()
                )));
            }
            // records are removed IN PLACE: the proof must state the same dispatch order
            if parse_dispatch_proof_method(proof.method())
                != Some(DispatchOrderStatement::SameOrder)
            {
                return Err(AssembleError::Retire(format!(
                    "the containment proof states {:?}; a retirement removes records in place (order=same)",
                    proof.method()
                )));
            }
            if r.retired().iter().any(|x| x == r.by())
                || !entries.iter().any(|e| e.line_sha256 == r.by())
            {
                return Err(AssembleError::Retire(format!(
                    "the superseding rule {} is not in the assembled set",
                    r.by()
                )));
            }
        }
        for id in r.retired() {
            if !entries.iter().any(|e| e.line_sha256 == *id) {
                return Err(AssembleError::Retire(format!(
                    "retired rule {id} is not assembled"
                )));
            }
            report.retired.push(id.clone());
        }
        // bind the proof to THIS assembly's tables in force before and after the retirement (int4; ladderfix2
        // `Retirement::check_tables`): both sets rebuilt here from the records assembly ships, digested by the ONE
        // definition `economy::table_set_digest`. A BOXED retirement is checked when it carries a proof.
        if full || r.tables().is_some() {
            let before =
                dispatch_set(entries.iter().map(|e| &e.record), &header, &scans[0].charts)?;
            let after = dispatch_set(
                entries
                    .iter()
                    .filter(|e| !r.retired().contains(&e.line_sha256))
                    .map(|e| &e.record),
                &header,
                &scans[0].charts,
            )?;
            r.check_tables(
                &DispatcherSource::new(&before),
                &DispatcherSource::new(&after),
                &frame,
            )
                .map_err(|e| {
                    AssembleError::Retire(format!(
                        "{e:?}: the containment proof is not about this assembly's table sets in force before / \
                         after the retirement (economy::table_set_digest)"
                    ))
                })?;
        }
        entries.retain(|e| !r.retired().contains(&e.line_sha256));
    }
    // (a FULL node's later exclusion hoists move only POINT records that APPLY at the key they are hoisted for, whose
    // selected set is that one key: coverage after them contains the proven coverage.)
    for r in &input.removals {
        if !entries.iter().any(|e| e.line_sha256 == r.rule_sha256) {
            return Err(AssembleError::Retire(format!(
                "removed rule {} is not assembled",
                r.rule_sha256
            )));
        }
        report.removed.push(r.rule_sha256.clone());
        entries.retain(|e| e.line_sha256 != r.rule_sha256);
    }
    // ---- ORDER (audit-accept-B C7): the SET of dispatched records before each campaign rule = its judged set
    {
        let mut prefix = SetDigest::default();
        for e in &entries {
            if let Some(ci) = e.campaign {
                let a = &input.accepted[ci];
                let (want, n) = a.in_force();
                if prefix.digest() != want || prefix.count() != n {
                    return Err(AssembleError::Order {
                        rule: e.line_sha256.clone(),
                        detail: format!(
                            "judged against {n} records in force ({want}); {} records precede it here ({}): \
                             re-judge it against the assembled prefix (plan D6)",
                            prefix.count(),
                            prefix.digest()
                        ),
                    });
                }
                report.order_checked += 1;
            }
            if e.dispatched {
                prefix.add(&e.line_sha256);
            }
        }
    }
    // ---- used-rule report (R20 boxed only, from a CLOSED closure; R33 full keeps everything)
    if let (Some(used), NodeTarget::Boxed { .. }) = (&input.used, &input.target) {
        entries.retain(|e| {
            let keep = used.contains(&e.line_sha256);
            if !keep {
                report.dropped_unused.push(e.line_sha256.clone());
            }
            keep
        });
    }
    // ---- exclusions: from the tokens only, made safe on the FINAL order (audit-accept-A m2, B C6)
    let terminals: BTreeSet<&Key> = input.terminals.iter().collect();
    let domains: Vec<Option<CaseDomain>> = entries
        .iter()
        .map(|e| domain_of(&e.record, &frame))
        .collect();
    let excl_of = |ei: usize| -> &[Exclusion] {
        entries[ei]
            .campaign
            .map(|ci| input.accepted[ci].exclusions())
            .unwrap_or(&[])
    };
    let excludes = |ei: usize, k: &Key| {
        excl_of(ei)
            .iter()
            .any(|x| matches!(&x.target, ExclusionTarget::Key(y) if y == k))
    };
    let mut order: Vec<usize> = (0..entries.len()).collect();
    let mut by_key: BTreeMap<Key, Vec<usize>> = BTreeMap::new();
    let mut blocking_raw: Vec<(usize, ExclusionTarget, String)> = Vec::new();
    for ei in 0..entries.len() {
        for ex in excl_of(ei) {
            match &ex.target {
                ExclusionTarget::Key(k) => by_key.entry(k.clone()).or_default().push(ei),
                ExclusionTarget::Locus { .. } => blocking_raw.push((
                    ei,
                    ex.target.clone(),
                    "a locus exclusion has no table form today (EC-8)".into(),
                )),
            }
        }
    }
    let mut safe_by: Vec<(usize, Key, String)> = Vec::new();
    let mut hoisted_entries: Vec<usize> = Vec::new();
    for (k, rules) in &by_key {
        if terminals.contains(k) {
            for &ei in rules {
                safe_by.push((ei, k.clone(), "terminal".into()));
            }
            continue;
        }
        let selects = |ei: usize| {
            domains[ei]
                .as_ref()
                .is_some_and(|d| d.applies(k).stage() != Stage::NotSelected)
        };
        let Some(fpos) = order.iter().position(|&ei| selects(ei)) else {
            for &ei in rules {
                safe_by.push((ei, k.clone(), "not_selected".into()));
            }
            continue;
        };
        let first = order[fpos];
        if !excludes(first, k) {
            for &ei in rules {
                safe_by.push((ei, k.clone(), format!("earlier_rule:{first}")));
            }
            continue;
        }
        // the first selecting record excludes k: hoist a POINT record that applies at k and does not exclude it
        let point = order[fpos + 1..].iter().copied().find(|&pj| {
            is_point(&entries[pj].record)
                && !excludes(pj, k)
                && domains[pj]
                    .as_ref()
                    .is_some_and(|d| d.applies(k).stage() == Stage::Applies)
        });
        match point {
            Some(pj) => {
                let from = order.iter().position(|&x| x == pj).expect("in order");
                order.remove(from);
                order.insert(fpos, pj);
                hoisted_entries.push(pj);
                for &ei in rules {
                    safe_by.push((ei, k.clone(), format!("hoisted_point_rule:{pj}")));
                }
            }
            None => {
                for &ei in rules {
                    if ei == first {
                        blocking_raw.push((
                            ei,
                            ExclusionTarget::Key(k.clone()),
                            "the first record selecting the key excludes it; no terminal, no point record to hoist"
                                .into(),
                        ));
                    } else {
                        safe_by.push((ei, k.clone(), format!("earlier_rule:{first}")));
                    }
                }
            }
        }
    }
    for (ei, k, how) in &safe_by {
        if let Some(pj) = how
            .strip_prefix("hoisted_point_rule:")
            .and_then(|x| x.parse::<usize>().ok())
            && !entries[pj].hoisted_for.contains(k)
        {
            entries[pj].hoisted_for.push(k.clone());
        }
        let _ = ei;
    }
    // apply the final order
    let mut slots: Vec<Option<Entry>> = entries.into_iter().map(Some).collect();
    let mut old_to_new: HashMap<usize, usize> = HashMap::new();
    let mut final_entries: Vec<Entry> = Vec::with_capacity(order.len());
    for &ei in &order {
        old_to_new.insert(ei, final_entries.len());
        final_entries.push(slots[ei].take().expect("each entry once"));
    }
    let entries = final_entries;
    for &pj in &hoisted_entries {
        let n = old_to_new[&pj];
        report
            .hoisted
            .push((n as u64, entries[n].hoisted_for.clone()));
    }
    let mut extras: Vec<Value> = Vec::new();
    for (ei, k, how) in &safe_by {
        report.exclusions_safe += 1;
        let n = old_to_new[ei];
        let how = match how.split_once(':') {
            Some((tag, x)) if tag == "earlier_rule" || tag == "hoisted_point_rule" => {
                format!(
                    "{tag}:{}",
                    x.parse::<usize>().map(|j| old_to_new[&j]).unwrap_or(0)
                )
            }
            _ => how.clone(),
        };
        extras.push(
            json!({"schema": EXTRAS_SCHEMA, "kind": "key_exclusion", "rule_n": n,
            "rule_sha256": entries[n].line_sha256, "key": k, "safe_by": how}),
        );
    }
    for (ei, target, why) in blocking_raw {
        let n = old_to_new[&ei];
        let (kind, key, eqs) = match &target {
            ExclusionTarget::Key(k) => ("key_exclusion", Some(k.clone()), None),
            ExclusionTarget::Locus { equations } => {
                ("locus_exclusion", None, Some(equations.clone()))
            }
        };
        extras.push(json!({"schema": EXTRAS_SCHEMA, "kind": kind, "rule_n": n,
            "rule_sha256": entries[n].line_sha256, "key": key, "equations": eqs, "safe_by": Value::Null, "why": why}));
        report.blocking.push(Blocking {
            rule_n: n as u64,
            rule_sha256: entries[n].line_sha256.clone(),
            target,
            why,
        });
    }
    let by_sha: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.line_sha256.as_str(), i))
        .collect();
    for c in &input.credits {
        report.credits_need_importer += 1;
        extras.push(
            json!({"schema": EXTRAS_SCHEMA, "kind": "symmetry_credit", "key": c.key,
            "element": c.element, "image": c.image, "rule_sha256": c.rule_sha256,
            "rule_n": by_sha.get(c.rule_sha256.as_str()), "descent_checked": true}),
        );
    }
    // ---- write
    std::fs::create_dir_all(out_dir)?;
    let node = &input.node;
    let table_path = out_dir.join(format!("{node}.assembled.table.jsonl"));
    let manifest_path = out_dir.join(format!("{node}.assembly.manifest.jsonl"));
    let extras_path = out_dir.join(format!("{node}.assembly.extras.jsonl"));
    let order_path = out_dir.join(format!("{node}.dispatch-order.json"));
    let order = DispatchOrder {
        schema: ORDER_SCHEMA,
        node: node.clone(),
        tables: order_tables,
        campaign_rules: entries
            .iter()
            .enumerate()
            .filter_map(|(n, e)| {
                e.campaign.map(|ci| OrderRule {
                    line_sha256: e.line_sha256.clone(),
                    position: e.position.clone(),
                    n: n as u64,
                    acceptance_record: input.accepted[ci].record().id().to_string(),
                })
            })
            .collect(),
        terminals: input.terminals.len(),
        credits: input.credits.len(),
    };
    let order_text = crate::schema::canon::canonical_json(
        &serde_json::to_value(&order).map_err(|e| AssembleError::Other(e.to_string()))?,
        false,
    )
    .map_err(AssembleError::Other)?;
    let mut oh = Sha256::new();
    oh.update(order_text.as_bytes());
    let order_digest = sha_tag(oh);
    std::fs::write(&order_path, format!("{order_text}\n"))?;

    // manifest first (its digest goes into the header)
    let mut man = String::new();
    man.push_str(
        &json!({"schema": MANIFEST_SCHEMA, "type": "header", "node": node, "order_digest": order_digest,
            "records": entries.len(), "renumber_index": input.renumber_index})
        .to_string(),
    );
    man.push('\n');
    for (n, e) in entries.iter().enumerate() {
        man.push_str(
            &json!({"n": n, "source": e.source_id, "source_sha256": e.source_sha256, "source_ordinal": e.ordinal,
                "byte_offset": e.offset, "bytes": e.bytes, "line_sha256": e.line_sha256, "index": e.record.index,
                "assembled_index": if input.renumber_index { json!(n) } else { json!(e.record.index) },
                "campaign": e.campaign.is_some(), "position": e.position,
                "acceptance_record": e.campaign.map(|ci| input.accepted[ci].record().id().to_string()),
                "hoisted_for": if e.hoisted_for.is_empty() { Value::Null } else { json!(e.hoisted_for) }})
            .to_string(),
        );
        man.push('\n');
    }
    let mut mh = Sha256::new();
    mh.update(man.as_bytes());
    let manifest_sha256 = sha_tag(mh);
    std::fs::write(&manifest_path, &man)?;
    let mut ex = String::new();
    for v in &extras {
        ex.push_str(&v.to_string());
        ex.push('\n');
    }
    std::fs::write(&extras_path, ex)?;

    let tmp = table_path.with_extension("jsonl.part");
    let mut th_sha = Sha256::new();
    {
        let mut w = BufWriter::with_capacity(1 << 20, File::create(&tmp)?);
        let mut put = |bytes: &[u8]| -> std::io::Result<()> {
            w.write_all(bytes)?;
            th_sha.update(bytes);
            Ok(())
        };
        let assembly = json!({"schema": ASSEMBLY_SCHEMA, "order_digest": order_digest,
            "manifest_sha256": manifest_sha256, "records": entries.len(), "node": node});
        let hl = header_line(
            scans[0].header_line.as_deref().unwrap_or_default(),
            &header,
            &assembly,
        )?;
        put(&hl)?;
        put(b"\n")?;
        for l in &scans[0].once_lines {
            put(l)?;
            put(b"\n")?;
        }
        let mut files: HashMap<PathBuf, File> = HashMap::new();
        for (n, e) in entries.iter().enumerate() {
            let f = match files.get_mut(&e.path) {
                Some(f) => f,
                None => {
                    files.insert(e.path.clone(), File::open(&e.path)?);
                    files.get_mut(&e.path).expect("just inserted")
                }
            };
            f.seek(SeekFrom::Start(e.offset))?;
            let d = copy_record(
                f,
                e.bytes,
                input.renumber_index.then_some((e.record.index, n as u64)),
                &mut put,
                &e.source_id,
            )?;
            if d != e.line_sha256 {
                return Err(AssembleError::Record {
                    source: e.source_id.clone(),
                    detail: "record changed while copying".into(),
                });
            }
            put(b"\n")?;
        }
        let mask = frame.mask_string();
        for k in &input.terminals {
            let line = json!({"type": "residual", "chart": mask, "key": key_text(k), "assembled_terminal": true});
            put(format!("{line}\n").as_bytes())?;
        }
        put(format!(
            "{}\n",
            json!({"type": "end", "records": entries.len(), "residuals": input.terminals.len(),
            "assembly": ASSEMBLY_SCHEMA})
        )
        .as_bytes())?;
        w.flush()?;
    }
    std::fs::rename(&tmp, &table_path)?;
    report.records = entries.len() as u64;
    report.campaign_records = entries.iter().filter(|e| e.campaign.is_some()).count() as u64;
    report.input_records = report.records - report.campaign_records;
    // ---- export readiness (audit-accept-A M3)
    if !report.blocking.is_empty() {
        report.not_ready.push(format!(
            "{} exclusions not safe in the table",
            report.blocking.len()
        ));
    }
    if report.credits_need_importer > 0 {
        report.not_ready.push(format!(
            "{} symmetry credits need the importer (EC-8)",
            report.credits_need_importer
        ));
    }
    if input.recipes > 0 {
        report
            .not_ready
            .push("recipes_need_engine_dispatcher".into());
    }
    if !report.undeclared_input_terminals.is_empty() {
        report.not_ready.push(format!(
            "{} input residual keys are not declared terminals",
            report.undeclared_input_terminals.len()
        ));
    }
    if full {
        let kr = entries
            .iter()
            .filter_map(|e| e.campaign)
            .filter(|&ci| *input.accepted[ci].scope() != Scope::FullDomain)
            .count();
        if kr > 0 {
            report.not_ready.push(format!(
                "{kr} campaign rules are certified on a key range only (scope key_range on a FULL node)"
            ));
        }
    }
    report.export_ready = report.not_ready.is_empty();
    Ok(AssemblyOutput {
        table: table_path,
        table_sha256: sha_tag(th_sha),
        manifest: manifest_path,
        manifest_sha256,
        extras: extras_path,
        order,
        order_digest,
        report,
    })
}

/// Check an assembled table FILE against its manifest (audit-accept-B M4: nothing checked the shipped file against the
/// dispatch order): the header's assembly provenance names the manifest digest and the record count; the rule records
/// appear in manifest order and each one, with its top-level index put back to the source index, has the manifest's
/// source line digest; the residual records are the declared terminals. `Ok(records)`.
pub fn verify_assembled(table: &Path, manifest: &Path) -> Result<usize, String> {
    let man = std::fs::read(manifest).map_err(|e| e.to_string())?;
    let mut mh = Sha256::new();
    mh.update(&man);
    let man_digest = sha_tag(mh);
    let man = String::from_utf8(man).map_err(|e| e.to_string())?;
    let mut lines = man.lines();
    let mhead: Value =
        serde_json::from_str(lines.next().ok_or("empty manifest")?).map_err(|e| e.to_string())?;
    let renumbered = mhead["renumber_index"].as_bool().unwrap_or(true);
    let rows: Vec<Value> = lines
        .map(|l| serde_json::from_str(l).map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    // two INDEPENDENT descriptors (a dup shares the offset): a streaming reader and one for the record copies
    let mut fr = File::open(table).map_err(|e| e.to_string())?;
    let mut rd = BufReader::with_capacity(1 << 20, File::open(table).map_err(|e| e.to_string())?);
    let mut buf = Vec::new();
    let mut off = 0u64;
    let mut n = 0usize;
    let mut header_seen = false;
    loop {
        buf.clear();
        let len = rd.read_until(b'\n', &mut buf).map_err(|e| e.to_string())? as u64;
        if len == 0 {
            break;
        }
        let start = off;
        off += len;
        let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let kind = line_kind(line)?.ok_or("a line without a type")?;
        match kind.as_str() {
            "header" => {
                let v: Value = serde_json::from_slice(line).map_err(|e| e.to_string())?;
                if v["assembly"]["manifest_sha256"] != man_digest.as_str()
                    || v["assembly"]["records"].as_u64() != Some(rows.len() as u64)
                {
                    return Err("header assembly provenance does not name this manifest".into());
                }
                header_seen = true;
            }
            "rule" => {
                let row = rows
                    .get(n)
                    .ok_or("more rule records than the manifest lists")?;
                let bytes = line.len() as u64;
                let src_index = row["index"].as_i64();
                fr.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
                // undo the renumbering on the head and hash the source bytes
                let mut out_hash = Sha256::new();
                let mut put = |b: &[u8]| -> std::io::Result<()> {
                    out_hash.update(b);
                    Ok(())
                };
                let assembled_index = row["assembled_index"].as_i64();
                let _ = copy_record(
                    &mut fr,
                    bytes,
                    match (renumbered, src_index) {
                        (true, Some(i)) => Some((
                            assembled_index,
                            u64::try_from(i).map_err(|_| format!("record {n}: negative index"))?,
                        )),
                        _ => None,
                    },
                    &mut put,
                    "assembled",
                )
                .map_err(|e| format!("record {n}: {e}"))?;
                if sha_tag(out_hash) != row["line_sha256"].as_str().unwrap_or("") {
                    return Err(format!(
                        "record {n} differs from the manifest's source line"
                    ));
                }
                n += 1;
            }
            _ => {}
        }
    }
    if !header_seen {
        return Err("no header".into());
    }
    if n != rows.len() {
        return Err(format!(
            "{n} rule records, the manifest lists {}",
            rows.len()
        ));
    }
    Ok(n)
}
