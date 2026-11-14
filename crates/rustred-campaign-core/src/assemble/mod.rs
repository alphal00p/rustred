//! TABLE ASSEMBLY (interface `interfaces/acceptance.md` section 5; plan 2.4.7 D1/D6/D7, 2.8, R20, R33).
//!
//! From a node's input tables, the accepted campaign rules (with their exclusions), symmetry credits, declared
//! terminals, retirements and removals: ONE dispatch order (digest), ONE `table_dump.v1` node table with ONE header
//! that the engine's `Table::load` reads (rule records in dispatch order = `find_rule` order, terminals as `residual`
//! records), a side manifest naming the source of every record, and `assembly.extras.jsonl` for what the engine cannot
//! express today (exclusions that are not made safe in the table, symmetry credits). Rule records are BYTE COPIES of
//! the source lines, streamed one at a time (records can be 100+ MB; the coefficient text is never re-serialised).

pub mod domaincheck;
pub mod recipes;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::{Value, json};

use crate::accept::{Exclusion, ExclusionTarget, NodeTarget, Position, PriorityClass};
use crate::closure::sha256::Sha256;
use crate::domain::{CaseDomain, DomainOptions, Stage};
use crate::economy::{CoverageState, RetireError, Retirement};
use crate::key::{Key, NodeFrame};
use crate::record::{ReadOptions, RecordReader, RhsMode, RuleRecord, TableHeader, TableLine};

pub const ASSEMBLY_SCHEMA: &str = "rustred.campaign.assembly.v1";
pub const MANIFEST_SCHEMA: &str = "rustred.campaign.assembly-manifest.v1";
pub const EXTRAS_SCHEMA: &str = "rustred.campaign.assembly-extras.v1";
pub const ORDER_SCHEMA: &str = "rustred.campaign.dispatch-order.v1";

/// An input table of the node with its spec priority (D1: fixed in the spec at campaign start).
#[derive(Clone, Debug)]
pub struct InputTable {
    pub id: String,
    pub path: PathBuf,
    pub priority: u32,
}

/// An accepted campaign rule: where its record line is, its digest, its position and its exclusions.
#[derive(Clone, Debug)]
pub struct AcceptedRule {
    pub source_id: String,
    pub source_path: PathBuf,
    pub byte_offset: u64,
    /// `sha256:` of the line (verified while copying).
    pub line_sha256: String,
    pub position: Position,
    pub exclusions: Vec<Exclusion>,
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

/// A D7 (i) removal: the rule was disproved at a served key.
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
    pub accepted: Vec<AcceptedRule>,
    pub terminals: Vec<Key>,
    pub credits: Vec<CreditRec>,
    /// Retirements (economy, R33: a FULL node's retirement carries a containment proof by construction).
    pub retirements: Vec<Retirement>,
    pub removals: Vec<Removal>,
    /// Copy records whose `dispatch` is not Partition (both matchers skip them).
    pub keep_skipped: bool,
    /// Rewrite the top-level `index` of every record to its assembled position (the ONLY byte change; unique
    /// indices = `find_rule`'s rule number, so `rr_proof --rule`, closure labels and domaincheck agree). The manifest
    /// keeps the source index and the source line digest.
    /// Rules that win at least one key of the final closure (line sha256): BOXED nodes ship only these (R20);
    /// FULL nodes keep everything (R33). `None` = no report attached.
    pub used: Option<BTreeSet<String>>,
    pub renumber_index: bool,
}

/// Errors (I/O and inconsistent inputs; never a silent pass).
#[derive(Debug)]
pub enum AssembleError {
    Io(std::io::Error),
    HeaderMismatch { field: String, table: String },
    Record { source: String, detail: String },
    Retire(String),
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

/// Scan of one input table: header value, zero_sectors / chart lines, rule entries.
struct Scanned {
    header: Option<Value>,
    zero_sectors: Option<Vec<u8>>,
    charts: Vec<Vec<u8>>,
    entries: Vec<Entry>,
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
        zero_sectors: None,
        charts: vec![],
        entries: vec![],
        sha256: String::new(),
        skipped: 0,
    };
    let mut ordinal = 0u64;
    loop {
        buf.clear();
        let n = rd.read_until(b'\n', &mut buf)? as u64;
        if n == 0 {
            break;
        }
        whole.update(&buf);
        let start = off;
        off += n;
        let line = buf.strip_suffix(b"\n").unwrap_or(&buf);
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        // the kind is read from the line prefix cheaply for big rule lines: every table_dump.v1 line starts with
        // {"type":"<kind>" as the engine writes it; anything else is parsed
        let kind = line_kind(line);
        match kind.as_deref() {
            Some("rule") => {
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
                    hoisted_for: vec![],
                });
            }
            Some("header") => {
                if out.header.is_some() {
                    return Err(AssembleError::Record {
                        source: t.id.clone(),
                        detail: "second header".into(),
                    });
                }
                out.header =
                    Some(
                        serde_json::from_slice(line).map_err(|e| AssembleError::Record {
                            source: t.id.clone(),
                            detail: e.to_string(),
                        })?,
                    );
            }
            Some("zero_sectors") => out.zero_sectors = Some(line.to_vec()),
            Some("chart") => out.charts.push(line.to_vec()),
            _ => {}
        }
    }
    out.sha256 = sha_tag(whole);
    for e in &mut out.entries {
        e.source_sha256 = out.sha256.clone();
    }
    Ok(out)
}

fn line_kind(line: &[u8]) -> Option<String> {
    let head = &line[..line.len().min(256)];
    let s = String::from_utf8_lossy(head);
    let i = s.find("\"type\"")?;
    let rest = s[i + 6..].trim_start().strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    Some(rest[..rest.find('"')?].to_string())
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

/// Replace the TOP-LEVEL `"index":<int>` of a record head by `n` (string-aware scan, depth 1 only); the value found
/// must equal `expected`. `None` if there is no such field in `head`.
pub fn splice_index(head: &[u8], expected: Option<i64>, n: u64) -> Option<Vec<u8>> {
    let pat = br#""index""#;
    let mut depth = 0i32;
    let mut i = 0usize;
    while i < head.len() {
        match head[i] {
            b'"' => {
                if depth == 1 && head[i..].starts_with(pat) {
                    let mut j = i + pat.len();
                    while j < head.len() && head[j].is_ascii_whitespace() {
                        j += 1;
                    }
                    if head.get(j) != Some(&b':') {
                        return None;
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
                    let v: i64 = std::str::from_utf8(&head[start..j]).ok()?.parse().ok()?;
                    if Some(v) != expected || j >= head.len() {
                        return None;
                    }
                    let mut out = Vec::with_capacity(head.len() + 8);
                    out.extend_from_slice(&head[..start]);
                    out.extend_from_slice(n.to_string().as_bytes());
                    out.extend_from_slice(&head[j..]);
                    return Some(out);
                }
                // skip the string
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
    None
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

/// Assemble into `out_dir` (files `<node>.assembled.table.jsonl`, `<node>.assembly.manifest.jsonl`,
/// `<node>.assembly.extras.jsonl`, `<node>.dispatch-order.json`).
pub fn assemble(input: &AssemblyInput, out_dir: &Path) -> Result<AssemblyOutput, AssembleError> {
    let mut report = AssemblyReport::default();
    // ---- inputs (sorted by spec priority, then id)
    let mut inputs = input.inputs.clone();
    inputs.sort_by(|a, b| (a.priority, &a.id).cmp(&(b.priority, &b.id)));
    let mut scans = Vec::new();
    for t in &inputs {
        scans.push(scan_input(t, input.keep_skipped)?);
    }
    let first = scans
        .iter()
        .position(|s| s.header.is_some())
        .ok_or_else(|| AssembleError::Other("no input table with a header".into()))?;
    let header = scans[first].header.clone().unwrap_or(Value::Null);
    for (s, t) in scans.iter().zip(&inputs) {
        if let Some(h) = &s.header {
            for f in [
                "denominators",
                "mask",
                "power_shifts",
                "family_source",
                "family",
            ] {
                if h.get(f) != header.get(f) {
                    return Err(AssembleError::HeaderMismatch {
                        field: f.into(),
                        table: t.id.clone(),
                    });
                }
            }
            if s.zero_sectors.is_some() && s.zero_sectors != scans[first].zero_sectors {
                return Err(AssembleError::HeaderMismatch {
                    field: "zero_sectors".into(),
                    table: t.id.clone(),
                });
            }
        }
    }
    let th = TableHeader::from_value(header.clone()).map_err(AssembleError::Other)?;
    let frame = NodeFrame::from_header(&th).map_err(|e| AssembleError::Other(e.to_string()))?;
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
    }
    // ---- campaign rules
    for (ci, a) in input.accepted.iter().enumerate() {
        let line = read_line_at(&a.source_path, a.byte_offset)?;
        let mut h = Sha256::new();
        h.update(&line);
        let d = sha_tag(h);
        if d != a.line_sha256 {
            return Err(AssembleError::Record {
                source: a.source_id.clone(),
                detail: format!(
                    "line at offset {} has digest {d}, expected {}",
                    a.byte_offset, a.line_sha256
                ),
            });
        }
        if a.position.class == PriorityClass::Input {
            return Err(AssembleError::Other(
                "a campaign rule in the input class".into(),
            ));
        }
        let rec = parse_rule_line(&line, &a.source_id)?;
        entries.push(Entry {
            source_id: a.source_id.clone(),
            path: a.source_path.clone(),
            source_sha256: String::new(),
            ordinal: 0,
            offset: a.byte_offset,
            bytes: line.len() as u64,
            line_sha256: d,
            record: rec,
            position: a.position.clone(),
            campaign: Some(ci),
            hoisted_for: vec![],
        });
    }
    // ---- retirements and removals (identity = line sha256)
    let mut gone: BTreeSet<String> = BTreeSet::new();
    let full = input.target == NodeTarget::Full;
    for r in &input.retirements {
        if full && (r.state() != CoverageState::Full || r.proof().is_none()) {
            return Err(AssembleError::Retire(format!(
                "{:?} on a FULL node needs a FULL retirement with a containment proof",
                RetireError::FullNodeNeedsProof
            )));
        }
        for id in r.retired() {
            gone.insert(id.clone());
            report.retired.push(id.clone());
        }
    }
    for r in &input.removals {
        gone.insert(r.rule_sha256.clone());
        report.removed.push(r.rule_sha256.clone());
    }
    entries.retain(|e| !gone.contains(&e.line_sha256));
    // ---- used-rule report (R20 boxed only; R33 full keeps everything)
    if let (Some(used), NodeTarget::Boxed { .. }) = (&input.used, &input.target) {
        entries.retain(|e| {
            let keep = used.contains(&e.line_sha256);
            if !keep {
                report.dropped_unused.push(e.line_sha256.clone());
            }
            keep
        });
    }
    // ---- dispatch order (D1, D6): stable sort by position
    entries.sort_by(|a, b| a.position.cmp(&b.position));
    // ---- exclusions: make safe in the table where possible
    let terminals: BTreeSet<&Key> = input.terminals.iter().collect();
    let domains: Vec<Option<CaseDomain>> = entries
        .iter()
        .map(|e| domain_of(&e.record, &frame))
        .collect();
    let mut hoist: BTreeMap<usize, usize> = BTreeMap::new(); // point entry -> before entry
    let mut extras: Vec<Value> = Vec::new();
    let mut safe_by: Vec<(usize, Key, String)> = Vec::new();
    for ri in 0..entries.len() {
        let Some(ci) = entries[ri].campaign else {
            continue;
        };
        for ex in &input.accepted[ci].exclusions {
            let k = match &ex.target {
                ExclusionTarget::Key(k) => k,
                ExclusionTarget::Locus { .. } => {
                    report.blocking.push(Blocking {
                        rule_n: ri as u64,
                        rule_sha256: entries[ri].line_sha256.clone(),
                        target: ex.target.clone(),
                        why: "a locus exclusion has no table form today (EC-8)".into(),
                    });
                    continue;
                }
            };
            if terminals.contains(k) {
                safe_by.push((ri, k.clone(), "terminal".into()));
                continue;
            }
            let earlier = (0..ri).find_map(|j| {
                domains[j]
                    .as_ref()
                    .map(|d| d.applies(k).stage())
                    .filter(|s| *s != Stage::NotSelected)
                    .map(|s| (j, s))
            });
            if let Some((j, s)) = earlier {
                safe_by.push((ri, k.clone(), format!("earlier_rule:{j}:{s:?}")));
                continue;
            }
            let point = (ri + 1..entries.len()).find(|&j| {
                is_point(&entries[j].record)
                    && domains[j]
                        .as_ref()
                        .is_some_and(|d| d.applies(k).stage() == Stage::Applies)
            });
            match point {
                Some(j) => {
                    let to = hoist.entry(j).or_insert(ri);
                    *to = (*to).min(ri);
                    entries[j].hoisted_for.push(k.clone());
                    safe_by.push((ri, k.clone(), format!("hoisted_point_rule:{j}")));
                }
                None => report.blocking.push(Blocking {
                    rule_n: ri as u64,
                    rule_sha256: entries[ri].line_sha256.clone(),
                    target: ex.target.clone(),
                    why: "no earlier rule, no terminal, no point record at the key".into(),
                }),
            }
        }
    }
    // apply hoists: move each point entry just before its target (targets in order)
    let n0 = entries.len();
    let mut placed: Vec<Option<Entry>> = entries.into_iter().map(Some).collect();
    let mut before: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (&p, &t) in &hoist {
        before.entry(t).or_default().push(p);
    }
    let mut final_entries: Vec<Entry> = Vec::with_capacity(n0);
    let mut old_to_new: HashMap<usize, usize> = HashMap::new();
    for i in 0..n0 {
        if let Some(ps) = before.get(&i) {
            for &p in ps {
                if let Some(e) = placed[p].take() {
                    old_to_new.insert(p, final_entries.len());
                    report
                        .hoisted
                        .push((final_entries.len() as u64, e.hoisted_for.clone()));
                    final_entries.push(e);
                }
            }
        }
        if let Some(e) = placed[i].take() {
            old_to_new.insert(i, final_entries.len());
            final_entries.push(e);
        }
    }
    let entries = final_entries;
    let by_sha: HashMap<&str, usize> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| (e.line_sha256.as_str(), i))
        .collect();
    for (ri, k, how) in &safe_by {
        report.exclusions_safe += 1;
        let n = old_to_new[ri];
        extras.push(
            json!({"schema": EXTRAS_SCHEMA, "kind": "key_exclusion", "rule_n": n,
            "rule_sha256": entries[n].line_sha256, "key": k, "safe_by": how}),
        );
    }
    for b in &mut report.blocking {
        b.rule_n = old_to_new[&(b.rule_n as usize)] as u64;
        let (kind, key, eqs) = match &b.target {
            ExclusionTarget::Key(k) => ("key_exclusion", Some(k.clone()), None),
            ExclusionTarget::Locus { equations } => {
                ("locus_exclusion", None, Some(equations.clone()))
            }
        };
        extras.push(json!({"schema": EXTRAS_SCHEMA, "kind": kind, "rule_n": b.rule_n,
            "rule_sha256": b.rule_sha256, "key": key, "equations": eqs, "safe_by": Value::Null, "why": b.why}));
    }
    for c in &input.credits {
        report.credits_need_importer += 1;
        extras.push(
            json!({"schema": EXTRAS_SCHEMA, "kind": "symmetry_credit", "key": c.key,
            "element": c.element, "image": c.image, "rule_sha256": c.rule_sha256,
            "rule_n": by_sha.get(c.rule_sha256.as_str()), "descent_checked": true}),
        );
    }
    // ---- write the table
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
            .filter(|(_, e)| e.campaign.is_some())
            .map(|(n, e)| OrderRule {
                line_sha256: e.line_sha256.clone(),
                position: e.position.clone(),
                n: n as u64,
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
            "records": entries.len()})
        .to_string(),
    );
    man.push('\n');
    for (n, e) in entries.iter().enumerate() {
        man.push_str(
            &json!({"n": n, "source": e.source_id, "source_sha256": e.source_sha256, "source_ordinal": e.ordinal,
                "byte_offset": e.offset, "bytes": e.bytes, "line_sha256": e.line_sha256, "index": e.record.index,
                "assembled_index": if input.renumber_index { json!(n) } else { json!(e.record.index) },
                "campaign": e.campaign.is_some(), "position": e.position,
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
        let put = |w: &mut BufWriter<File>, bytes: &[u8], h: &mut Sha256| -> std::io::Result<()> {
            w.write_all(bytes)?;
            h.update(bytes);
            Ok(())
        };
        let mut hv = header.clone();
        hv["assembly"] = json!({"schema": ASSEMBLY_SCHEMA, "order_digest": order_digest,
            "manifest_sha256": manifest_sha256, "records": entries.len(), "node": node});
        put(&mut w, format!("{hv}\n").as_bytes(), &mut th_sha)?;
        if let Some(z) = &scans[first].zero_sectors {
            put(&mut w, z, &mut th_sha)?;
            put(&mut w, b"\n", &mut th_sha)?;
        }
        for c in &scans[first].charts {
            put(&mut w, c, &mut th_sha)?;
            put(&mut w, b"\n", &mut th_sha)?;
        }
        let n_of: HashMap<(PathBuf, u64), u64> = entries
            .iter()
            .enumerate()
            .map(|(n, e)| ((e.path.clone(), e.offset), n as u64))
            .collect();
        let mut files: HashMap<PathBuf, File> = HashMap::new();
        let mut chunk = vec![0u8; 1 << 20];
        for e in &entries {
            let f = match files.get_mut(&e.path) {
                Some(f) => f,
                None => {
                    files.insert(e.path.clone(), File::open(&e.path)?);
                    files.get_mut(&e.path).expect("just inserted")
                }
            };
            f.seek(SeekFrom::Start(e.offset))?;
            let mut left = e.bytes;
            let mut lh = Sha256::new();
            if input.renumber_index {
                let m = (left as usize).min(chunk.len());
                f.read_exact(&mut chunk[..m])?;
                lh.update(&chunk[..m]);
                let head = splice_index(
                    &chunk[..m],
                    e.record.index,
                    n_of[&(e.path.clone(), e.offset)],
                )
                .ok_or_else(|| AssembleError::Record {
                    source: e.source_id.clone(),
                    detail: "no top-level \"index\" field in the record head (renumbering refused)"
                        .into(),
                })?;
                put(&mut w, &head, &mut th_sha)?;
                left -= m as u64;
            }
            while left > 0 {
                let m = (left as usize).min(chunk.len());
                f.read_exact(&mut chunk[..m])?;
                lh.update(&chunk[..m]);
                put(&mut w, &chunk[..m], &mut th_sha)?;
                left -= m as u64;
            }
            if sha_tag(lh) != e.line_sha256 {
                return Err(AssembleError::Record {
                    source: e.source_id.clone(),
                    detail: "record changed while copying".into(),
                });
            }
            put(&mut w, b"\n", &mut th_sha)?;
        }
        let mask = frame.mask_string();
        for k in &input.terminals {
            let line = json!({"type": "residual", "chart": mask, "key": key_text(k), "assembled_terminal": true});
            put(&mut w, format!("{line}\n").as_bytes(), &mut th_sha)?;
        }
        w.flush()?;
    }
    std::fs::rename(&tmp, &table_path)?;
    report.records = entries.len() as u64;
    report.campaign_records = entries.iter().filter(|e| e.campaign.is_some()).count() as u64;
    report.input_records = report.records - report.campaign_records;
    report.export_ready = report.blocking.is_empty() && report.credits_need_importer == 0;
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
