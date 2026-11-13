//! Coverage of keys by prioritized rule tables: first-match dispatch (the default) and, as a separate and labelled
//! mode, union coverage.
//!
//! - A [`Table`] is a named list of [`CaseDomain`]s in record order, each with a label (the record's `i`, `index` or
//!   bank id) so that every credit names its rule, plus its [`TableProvenance`] (source format, where the targets
//!   come from, whether witness loci were carried, which records were skipped because the engine never selects them).
//!   The loaders [`Table::from_table_file`] and [`Table::from_bank`] build the frame from the header and record all of
//!   this; [`Table::new`] + [`Table::push`] build a table by hand (provenance `Manual`).
//! - The PRIORITY of the tables is data given by the caller ([`Dispatcher::new`]; DESIGN-COMMENTS B6) and must list
//!   every table exactly once.
//! - First match ([`Dispatcher::first_match`]) follows the engine's `find_rule`: tables in priority order, records in
//!   record order; a record whose verdict is [`Stage::NotSelected`] is passed over; the FIRST other record ends the
//!   search: [`Dispatch::Covered`] if it applies, [`Dispatch::Blocked`] if it is selected but unusable at the key
//!   (pole, recenter bound, elided witness loci), [`Dispatch::Undecided`] on an evaluation error. A key outside the
//!   common chart of the tables is [`Dispatch::OutsideChart`] (the engine never dispatches such a key). Only
//!   `Covered` is coverage; [`Dispatch::is_open`] is true for every other verdict.
//! - Caller mistakes (key of the wrong length, inconsistent tables, a priority that is not a permutation) are typed
//!   errors ([`CoverError`]), never `Uncovered`.
//! - [`Dispatcher::union_cover`] lists EVERY applying rule (union semantics: no dispatch order). It is what the
//!   checker's partition tools call "covered"; it is NOT the engine's dispatch.
//!
//! Index: the records of all tables are flattened in dispatch order and grouped by their fixed-slot pattern; per
//! pattern a hash map from the fixed values to the record list. For a key the candidate records are the union of the
//! lists of its projections, evaluated in dispatch order. A record not among the candidates has a differing fixed
//! slot, i.e. verdict `NotSelected`, so dropping it cannot change the first match. [`Dispatcher::first_match_slow`]
//! is the plain record-by-record reference scan (no index), used by the differential tests.

use std::collections::HashMap;
use std::time::Instant;

use crate::domain::{CaseDomain, DomainError, DomainOptions, Stage, TargetSource, Verdict};
use crate::key::NodeFrame;
use crate::record::{BankPredicate, RecordFormat, TableFile};

/// Format of the source a table was loaded from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFormat {
    /// rr_node `table_dump.v1`.
    TableDump,
    /// Case-domain export (no target, no witness loci).
    CaseExport,
    /// T162 bank predicates.
    Bank,
    /// Built by hand with [`Table::push`].
    Manual,
}

/// Whether the witness loci of the records were checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WitnessProvenance {
    /// Every record carries `witness_loci` and they were checked.
    Checked,
    /// `with` of `total` records carry `witness_loci` (checked where present); the others may hide elided loci.
    Partial { with: usize, total: usize },
    /// The source carries no witness loci (case-domain exports): an elided record cannot be detected.
    NotCarried,
    /// The caller turned the check off ([`DomainOptions::witness`] = false).
    Disabled,
    /// Not applicable (bank predicates carry their witness loci as pole loci; manual tables).
    NotApplicable,
}

/// A record the loader did not turn into a candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkippedRecord {
    pub line_no: usize,
    pub label: i64,
    pub dispatch: String,
}

/// Where a table came from and what was assumed while loading it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableProvenance {
    /// Path or description given by the caller.
    pub source: String,
    pub format: SourceFormat,
    /// Targets read from the records, or the caller's statement that they are canonical by construction.
    pub targets: Option<TargetSource>,
    pub witness: WitnessProvenance,
    /// Records with `dispatch` other than `Partition`: never candidates (the engine's `Table::load` skips them).
    pub skipped_not_dispatched: Vec<SkippedRecord>,
}

impl TableProvenance {
    fn manual(name: &str) -> Self {
        TableProvenance {
            source: name.to_string(),
            format: SourceFormat::Manual,
            targets: None,
            witness: WitnessProvenance::NotApplicable,
            skipped_not_dispatched: vec![],
        }
    }
}

/// A named rule table.
#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    arity: usize,
    pub domains: Vec<CaseDomain>,
    /// One label per domain (record `i`, `index` or bank id).
    pub labels: Vec<i64>,
    pub provenance: TableProvenance,
}

/// Errors of the table loaders.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadError {
    /// The file has no header (the frame is built from the header).
    NoHeader,
    Frame(crate::key::FrameError),
    /// A `chart` line other than the node mask, or with guards other than `n>=1` / `n<=0` (the engine refuses both).
    Chart(String),
    /// A record without `i` and `index` (no label).
    NoLabel {
        line_no: usize,
    },
    /// A record whose domain cannot be built.
    Record {
        line_no: usize,
        err: DomainError,
    },
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NoHeader => write!(f, "table without header"),
            LoadError::Frame(e) => write!(f, "{e}"),
            LoadError::Chart(s) => write!(f, "chart line: {s}"),
            LoadError::NoLabel { line_no } => write!(f, "line {line_no}: record without i/index"),
            LoadError::Record { line_no, err } => write!(f, "line {line_no}: {err}"),
        }
    }
}
impl std::error::Error for LoadError {}

impl Table {
    /// An empty table built by hand (provenance `Manual`).
    pub fn new(name: impl Into<String>, arity: usize) -> Self {
        let name = name.into();
        Table {
            provenance: TableProvenance::manual(&name),
            name,
            arity,
            domains: vec![],
            labels: vec![],
        }
    }
    /// Append a domain (validated, arity checked).
    pub fn push(&mut self, label: i64, d: CaseDomain) -> Result<(), CoverError> {
        if d.arity != self.arity {
            return Err(CoverError::Arity(format!(
                "domain of arity {} in table {} of arity {}",
                d.arity, self.name, self.arity
            )));
        }
        d.validate()
            .map_err(|e| CoverError::Invalid(format!("table {} label {label}: {e}", self.name)))?;
        self.labels.push(label);
        self.domains.push(d);
        Ok(())
    }
    pub fn arity(&self) -> usize {
        self.arity
    }
    pub fn len(&self) -> usize {
        self.domains.len()
    }
    pub fn is_empty(&self) -> bool {
        self.domains.is_empty()
    }
    /// Records skipped because their `dispatch` is not `Partition`.
    pub fn skipped_count(&self) -> usize {
        self.provenance.skipped_not_dispatched.len()
    }

    /// Load a table file read by [`crate::record::read_table_file`] (either JSONL format). The frame is built from
    /// the header ([`NodeFrame::from_header`]); `chart` lines must equal the mask with the standard guards; labels are
    /// the records' `i`, else `index`. Records with `dispatch` other than `Partition` are recorded in the provenance
    /// and are not candidates. Any other unreadable record is an error (fail closed).
    pub fn from_table_file(
        name: impl Into<String>,
        source: impl Into<String>,
        tf: &TableFile,
        opts: DomainOptions,
    ) -> Result<Table, LoadError> {
        let h = tf.header.as_ref().ok_or(LoadError::NoHeader)?;
        let frame = NodeFrame::from_header(h).map_err(LoadError::Frame)?;
        let mask = frame.mask_string();
        for (c, guards) in &tf.charts {
            if *c != mask {
                return Err(LoadError::Chart(format!(
                    "{c} differs from the node mask {mask}"
                )));
            }
            if guards.len() != frame.arity()
                || guards
                    .iter()
                    .enumerate()
                    .any(|(i, g)| g != if frame.is_line(i) { "n>=1" } else { "n<=0" })
            {
                return Err(LoadError::Chart(format!("unsupported guards {guards:?}")));
            }
        }
        let name = name.into();
        let mut t = Table::new(name, frame.arity());
        let fmt = match tf.rules.first().map(|r| r.format) {
            Some(RecordFormat::CaseExport) => SourceFormat::CaseExport,
            _ => SourceFormat::TableDump,
        };
        let with_w = tf.rules.iter().filter(|r| r.witness_loci.is_some()).count();
        t.provenance = TableProvenance {
            source: source.into(),
            format: fmt,
            targets: Some(opts.targets),
            witness: if !opts.witness {
                WitnessProvenance::Disabled
            } else if with_w == tf.rules.len() && with_w > 0 {
                WitnessProvenance::Checked
            } else if with_w == 0 {
                WitnessProvenance::NotCarried
            } else {
                WitnessProvenance::Partial {
                    with: with_w,
                    total: tf.rules.len(),
                }
            },
            skipped_not_dispatched: vec![],
        };
        for r in &tf.rules {
            let label =
                r.i.or(r.index)
                    .ok_or(LoadError::NoLabel { line_no: r.line_no })?;
            match CaseDomain::from_rule_record(r, &frame, opts) {
                Ok(d) => t.push(label, d).map_err(|e| LoadError::Record {
                    line_no: r.line_no,
                    err: DomainError::Invalid(e.to_string()),
                })?,
                Err(DomainError::NotDispatched { dispatch }) => {
                    t.provenance.skipped_not_dispatched.push(SkippedRecord {
                        line_no: r.line_no,
                        label,
                        dispatch,
                    })
                }
                Err(err) => {
                    return Err(LoadError::Record {
                        line_no: r.line_no,
                        err,
                    });
                }
            }
        }
        Ok(t)
    }

    /// Load T162 bank predicates (label = `global_ordinal0`, in the given order). The bank carries no target:
    /// `targets` must be the caller's [`TargetSource::CanonicalByConstruction`] statement.
    pub fn from_bank(
        name: impl Into<String>,
        source: impl Into<String>,
        preds: &[BankPredicate],
        arity: usize,
        targets: TargetSource,
    ) -> Result<Table, LoadError> {
        let mut t = Table::new(name, arity);
        t.provenance = TableProvenance {
            source: source.into(),
            format: SourceFormat::Bank,
            targets: Some(targets),
            witness: WitnessProvenance::NotApplicable,
            skipped_not_dispatched: vec![],
        };
        for p in preds {
            let d = CaseDomain::from_bank_predicate(p, arity, targets)
                .map_err(|err| LoadError::Record { line_no: 0, err })?;
            t.push(p.global_ordinal0, d)
                .map_err(|e| LoadError::Record {
                    line_no: 0,
                    err: DomainError::Invalid(e.to_string()),
                })?;
        }
        Ok(t)
    }
}

/// A credited rule: table (index into the dispatcher's table list), record position in the table, record label.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleRef {
    pub table: usize,
    pub record: usize,
    pub label: i64,
}

/// Union mode: the applying rules, and the rules selected but blocked or undecided (with their verdicts).
pub type UnionCover = (Vec<RuleRef>, Vec<(RuleRef, Verdict)>);

/// Result of first-match dispatch at one key. Only `Covered` is coverage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dispatch {
    Covered(RuleRef),
    /// No rule is selected.
    Uncovered,
    /// The first selected rule cannot be used at the key (`reason` is its verdict: pole, recenter bound, elided
    /// witness loci, chart mismatch). The key is OPEN; the engine would fail on it, not try the next rule.
    Blocked {
        at: RuleRef,
        reason: Verdict,
    },
    /// Whether the rule at `at` is selected could not be decided (evaluation error). The key is OPEN.
    Undecided {
        at: RuleRef,
        verdict: Verdict,
    },
    /// The key is outside the common chart of the tables (first violating slot): not dispatched at all.
    OutsideChart {
        slot: usize,
    },
}

impl Dispatch {
    pub fn winner(&self) -> Option<RuleRef> {
        match self {
            Dispatch::Covered(r) => Some(*r),
            _ => None,
        }
    }
    pub fn is_covered(&self) -> bool {
        matches!(self, Dispatch::Covered(_))
    }
    /// Every verdict other than `Covered`: the key needs a rule (gap lists and closure treat it as open).
    pub fn is_open(&self) -> bool {
        !self.is_covered()
    }
}

/// Errors of the dispatcher (setup and caller mistakes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoverError {
    /// The priority list is not a permutation of the table indices.
    BadPriority(String),
    /// Tables or domains of different arity, or no table at all.
    Arity(String),
    /// A key whose length differs from the dispatcher arity.
    KeyArity { expected: usize, got: usize },
    /// Inconsistent domains (validation failed, labels missing, charts that differ).
    Invalid(String),
}

impl std::fmt::Display for CoverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoverError::BadPriority(s) => write!(f, "bad priority: {s}"),
            CoverError::Arity(s) => write!(f, "arity: {s}"),
            CoverError::KeyArity { expected, got } => {
                write!(f, "key of arity {got}, dispatcher arity {expected}")
            }
            CoverError::Invalid(s) => write!(f, "invalid tables: {s}"),
        }
    }
}
impl std::error::Error for CoverError {}

/// Small multiplicative hasher for the fixed-value index (keys are short `i32` vectors; no DoS concern).
#[derive(Default, Clone, Copy)]
struct FxHasher(u64);

impl std::hash::Hasher for FxHasher {
    fn finish(&self) -> u64 {
        // splitmix64 finaliser: spreads the state over all bits (the table uses both high and low bits)
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    fn write(&mut self, bytes: &[u8]) {
        let (chunks, r) = bytes.as_chunks::<8>();
        for c in chunks {
            self.mix(u64::from_le_bytes(*c));
        }
        if !r.is_empty() {
            let mut b = [0u8; 8];
            b[..r.len()].copy_from_slice(r);
            self.mix(u64::from_le_bytes(b));
        }
    }
    fn write_i32(&mut self, i: i32) {
        self.mix(i as u32 as u64);
    }
    fn write_usize(&mut self, i: usize) {
        self.mix(i as u64);
    }
}

impl FxHasher {
    #[inline]
    fn mix(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(5) ^ x).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

type FxBuild = std::hash::BuildHasherDefault<FxHasher>;

struct Pattern {
    slots: Vec<usize>,
    map: HashMap<Vec<i32>, Vec<u32>, FxBuild>,
}

/// Prioritized tables with a fixed-slot index.
pub struct Dispatcher {
    tables: Vec<Table>,
    priority: Vec<usize>,
    /// Flattened entries in dispatch order: (table, record).
    entries: Vec<(u32, u32)>,
    patterns: Vec<Pattern>,
    arity: usize,
    /// The common chart of the domains that carry one.
    chart: Option<Vec<bool>>,
}

/// Batch result.
#[derive(Clone, Debug)]
pub struct BatchReport {
    pub results: Vec<Dispatch>,
    /// Wins per table (indexed like the table list, not the priority).
    pub wins: Vec<usize>,
    pub covered: usize,
    pub uncovered: usize,
    pub blocked: usize,
    pub undecided: usize,
    pub outside_chart: usize,
    pub seconds: f64,
}

impl BatchReport {
    /// Keys that are not covered (uncovered + blocked + undecided + outside the chart).
    pub fn open(&self) -> usize {
        self.uncovered + self.blocked + self.undecided + self.outside_chart
    }
}

impl Dispatcher {
    /// `priority` lists every table index exactly once, highest priority first. All tables must have the same
    /// arity (at least one table); every domain is validated; the domains that carry a chart must agree on it.
    pub fn new(tables: Vec<Table>, priority: Vec<usize>) -> Result<Self, CoverError> {
        let mut seen = vec![false; tables.len()];
        for &p in &priority {
            if p >= tables.len() || seen[p] {
                return Err(CoverError::BadPriority(format!(
                    "{priority:?} for {} tables",
                    tables.len()
                )));
            }
            seen[p] = true;
        }
        if seen.iter().any(|s| !s) {
            return Err(CoverError::BadPriority(format!(
                "{priority:?} does not list all {} tables",
                tables.len()
            )));
        }
        let arity = tables
            .first()
            .map(Table::arity)
            .ok_or_else(|| CoverError::Arity("no table".into()))?;
        let mut chart: Option<Vec<bool>> = None;
        for t in &tables {
            if t.arity != arity {
                return Err(CoverError::Arity(format!(
                    "table {} of arity {}, expected {arity}",
                    t.name, t.arity
                )));
            }
            if t.labels.len() != t.domains.len() {
                return Err(CoverError::Invalid(format!(
                    "table {}: {} labels for {} domains",
                    t.name,
                    t.labels.len(),
                    t.domains.len()
                )));
            }
            for (d, l) in t.domains.iter().zip(&t.labels) {
                if d.arity != arity {
                    return Err(CoverError::Arity(format!(
                        "table {} label {l}: domain of arity {}",
                        t.name, d.arity
                    )));
                }
                d.validate()
                    .map_err(|e| CoverError::Invalid(format!("table {} label {l}: {e}", t.name)))?;
                if let Some(c) = &d.chart {
                    match &chart {
                        None => chart = Some(c.clone()),
                        Some(c0) if c0 != c => {
                            return Err(CoverError::Invalid(format!(
                                "table {} label {l}: chart differs from the other domains",
                                t.name
                            )));
                        }
                        _ => {}
                    }
                }
            }
        }
        let mut entries = vec![];
        for &t in &priority {
            for r in 0..tables[t].len() {
                entries.push((t as u32, r as u32));
            }
        }
        let mut pidx: HashMap<Vec<usize>, usize> = HashMap::new();
        let mut patterns: Vec<Pattern> = vec![];
        for (e, &(t, r)) in entries.iter().enumerate() {
            let d = &tables[t as usize].domains[r as usize];
            let slots = d.fixed_slots();
            let vals: Vec<i32> = slots.iter().map(|&s| d.fixed[s].expect("fixed")).collect();
            let pi = *pidx.entry(slots.clone()).or_insert_with(|| {
                patterns.push(Pattern {
                    slots,
                    map: HashMap::default(),
                });
                patterns.len() - 1
            });
            patterns[pi].map.entry(vals).or_default().push(e as u32);
        }
        Ok(Dispatcher {
            tables,
            priority,
            entries,
            patterns,
            arity,
            chart,
        })
    }

    /// The same tables under another priority.
    pub fn with_priority(&self, priority: Vec<usize>) -> Result<Self, CoverError> {
        Dispatcher::new(self.tables.clone(), priority)
    }
    pub fn tables(&self) -> &[Table] {
        &self.tables
    }
    pub fn priority(&self) -> &[usize] {
        &self.priority
    }
    pub fn arity(&self) -> usize {
        self.arity
    }
    /// The common chart of the domains (`None` if no domain carries one).
    pub fn chart(&self) -> Option<&[bool]> {
        self.chart.as_deref()
    }
    pub fn n_patterns(&self) -> usize {
        self.patterns.len()
    }

    fn check_key(&self, key: &[i32]) -> Result<(), CoverError> {
        if key.len() != self.arity {
            return Err(CoverError::KeyArity {
                expected: self.arity,
                got: key.len(),
            });
        }
        Ok(())
    }

    fn outside_chart(&self, key: &[i32]) -> Option<usize> {
        let mask = self.chart.as_ref()?;
        key.iter()
            .enumerate()
            .position(|(i, &x)| (mask[i] && x < 1) || (!mask[i] && x > 0))
    }

    fn candidates(&self, key: &[i32], out: &mut Vec<u32>) {
        out.clear();
        let mut proj = Vec::with_capacity(self.arity);
        for p in &self.patterns {
            proj.clear();
            proj.extend(p.slots.iter().map(|&s| key[s]));
            if let Some(v) = p.map.get(&proj) {
                out.extend_from_slice(v);
            }
        }
        out.sort_unstable();
    }

    fn rref(&self, e: u32) -> RuleRef {
        let (t, r) = self.entries[e as usize];
        RuleRef {
            table: t as usize,
            record: r as usize,
            label: self.tables[t as usize].labels[r as usize],
        }
    }

    fn verdict(&self, e: u32, key: &[i32]) -> Verdict {
        let (t, r) = self.entries[e as usize];
        self.tables[t as usize].domains[r as usize].applies(key)
    }

    /// The engine's selection over entries in dispatch order (the key is already checked).
    fn select(&self, key: &[i32], entries: impl Iterator<Item = u32>) -> Dispatch {
        if let Some(slot) = self.outside_chart(key) {
            return Dispatch::OutsideChart { slot };
        }
        for e in entries {
            let v = self.verdict(e, key);
            match v.stage() {
                Stage::Applies => return Dispatch::Covered(self.rref(e)),
                Stage::NotSelected => {}
                Stage::Blocked => {
                    return Dispatch::Blocked {
                        at: self.rref(e),
                        reason: v,
                    };
                }
                Stage::Undecided => {
                    return Dispatch::Undecided {
                        at: self.rref(e),
                        verdict: v,
                    };
                }
            }
        }
        Dispatch::Uncovered
    }

    /// First-match dispatch at `key` (fast path: fixed-value index).
    pub fn first_match(&self, key: &[i32]) -> Result<Dispatch, CoverError> {
        self.check_key(key)?;
        let mut c = Vec::new();
        Ok(self.first_match_with(key, &mut c))
    }

    fn first_match_with(&self, key: &[i32], c: &mut Vec<u32>) -> Dispatch {
        self.candidates(key, c);
        self.select(key, c.iter().copied())
    }

    /// SLOW reference: first-match dispatch by a plain scan of every record in dispatch order (no index). For
    /// differential tests of [`Dispatcher::first_match`].
    pub fn first_match_slow(&self, key: &[i32]) -> Result<Dispatch, CoverError> {
        self.check_key(key)?;
        Ok(self.select(key, 0..self.entries.len() as u32))
    }

    /// UNION coverage (not dispatch): every rule that applies at `key`, in priority order; second list: rules that
    /// are selected but blocked or undecided at the key, with their verdicts. A key outside the common chart has
    /// no rule in either list.
    pub fn union_cover(&self, key: &[i32]) -> Result<UnionCover, CoverError> {
        self.check_key(key)?;
        let mut c = Vec::new();
        let (mut yes, mut open) = (vec![], vec![]);
        if self.outside_chart(key).is_some() {
            return Ok((yes, open));
        }
        self.candidates(key, &mut c);
        for &e in &c {
            let v = self.verdict(e, key);
            match v.stage() {
                Stage::Applies => yes.push(self.rref(e)),
                Stage::NotSelected => {}
                Stage::Blocked | Stage::Undecided => open.push((self.rref(e), v)),
            }
        }
        Ok((yes, open))
    }

    /// First-match dispatch over a key list, `threads` worker threads (1..=4 recommended). Every key is checked
    /// first; a key of the wrong length is an error for the whole batch.
    pub fn batch(&self, keys: &[Vec<i32>], threads: usize) -> Result<BatchReport, CoverError> {
        self.run_batch(keys, threads, false)
    }

    /// SLOW reference of [`Dispatcher::batch`] ([`Dispatcher::first_match_slow`] per key).
    pub fn batch_slow(&self, keys: &[Vec<i32>], threads: usize) -> Result<BatchReport, CoverError> {
        self.run_batch(keys, threads, true)
    }

    fn run_batch(
        &self,
        keys: &[Vec<i32>],
        threads: usize,
        slow: bool,
    ) -> Result<BatchReport, CoverError> {
        for k in keys {
            self.check_key(k)?;
        }
        let t0 = Instant::now();
        let threads = threads.max(1);
        let chunk = keys.len().div_ceil(threads).max(1);
        let mut results: Vec<Dispatch> = Vec::with_capacity(keys.len());
        std::thread::scope(|s| {
            let hs: Vec<_> = keys
                .chunks(chunk)
                .map(|ks| {
                    s.spawn(move || {
                        let mut c = Vec::new();
                        ks.iter()
                            .map(|k| {
                                if slow {
                                    self.select(k, 0..self.entries.len() as u32)
                                } else {
                                    self.first_match_with(k, &mut c)
                                }
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            for h in hs {
                results.extend(h.join().expect("worker"));
            }
        });
        let mut rep = BatchReport {
            results: vec![],
            wins: vec![0; self.tables.len()],
            covered: 0,
            uncovered: 0,
            blocked: 0,
            undecided: 0,
            outside_chart: 0,
            seconds: 0.0,
        };
        for r in &results {
            match r {
                Dispatch::Covered(w) => {
                    rep.wins[w.table] += 1;
                    rep.covered += 1;
                }
                Dispatch::Uncovered => rep.uncovered += 1,
                Dispatch::Blocked { .. } => rep.blocked += 1,
                Dispatch::Undecided { .. } => rep.undecided += 1,
                Dispatch::OutsideChart { .. } => rep.outside_chart += 1,
            }
        }
        rep.results = results;
        rep.seconds = t0.elapsed().as_secs_f64();
        Ok(rep)
    }
}
