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

use std::collections::{BTreeSet, HashMap};
use std::time::Instant;

use crate::domain::{
    CaseDomain, DomainError, DomainOptions, Stage, TargetMap, TargetSource, Verdict,
    is_canonical_target, parse_target, record_fixed as fixed_of,
};
use crate::key::NodeFrame;
use crate::record::{BankPredicate, RecordFormat, RuleRecord, TableFile};

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

/// What the SOURCE shows about one record's witness loci (re-audit RA-MAJOR-1). Dispatch ENFORCES it: a selected
/// record whose status is not `Complete` blocks the key ([`Dispatch::Blocked`] with [`Verdict::Elided`] or
/// [`Verdict::WitnessNotCarried`]) unless the caller turned the check off ([`DomainOptions::witness`] = false, visible
/// as [`Assurance::CheckDisabled`] in every credit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WitnessStatus {
    /// The witness loci are present and not elided (raw `table_dump.v1` record, bank predicate), or a witness
    /// attestation covers the record ([`Attestation`]).
    Complete,
    /// The witness loci hold the literal `"elided"` (ruling F1: unusable).
    Elided,
    /// The source format has no witness loci (case-domain export; a raw record without the field) and no attestation
    /// covers the record.
    NotCarried,
}

/// On what a property of a record (its witness loci, its target) rests. Carried by every [`RuleRef`], so every
/// credit says it (re-audit RA-MAJOR-1/2: reports can count credits resting on unchecked attestations).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Assurance {
    /// Read from the record itself (a raw target; witness loci present in the record).
    Record,
    /// A caller's [`Attestation`], CHECKED record by record against the raw table given with it.
    AttestedChecked,
    /// A caller's statement that could not be checked: an [`Attestation`] without its raw table, or the legacy
    /// [`TargetSource::CanonicalByConstruction`] text.
    AttestedUnchecked,
    /// Built by hand ([`Table::push`]): the caller constructed the domain, loci included.
    Caller,
    /// The caller turned the witness check off ([`DomainOptions::witness`] = false; tests, legacy reference numbers).
    CheckDisabled,
}

/// The basis of a credit: what the record's witness status and target rest on, and whether the chart was tested.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CreditBasis {
    pub witness: Assurance,
    pub target: Assurance,
    /// The dispatcher tested the key against a common chart before dispatch (false: no domain carries a chart, e.g.
    /// a bank-only dispatcher or [`DomainOptions::chart`] = false, and no chart was given with
    /// [`Dispatcher::with_chart`]; a key the engine never dispatches may then be credited: re-audit RA minor N-m4).
    pub chart_checked: bool,
}

impl CreditBasis {
    /// Witness or target rests on an attestation that was not checked.
    pub fn rests_on_unchecked_attestation(&self) -> bool {
        self.witness == Assurance::AttestedUnchecked || self.target == Assurance::AttestedUnchecked
    }
    /// The strict basis: witness and target read from the record or checked against the raw table, chart tested.
    pub fn is_strict(&self) -> bool {
        let ok = |a: Assurance| matches!(a, Assurance::Record | Assurance::AttestedChecked);
        ok(self.witness) && ok(self.target) && self.chart_checked
    }
}

/// Per-record data of a table, parallel to [`Table::domains`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RecordMeta {
    /// 0-based position of the record among the RULE records of the source (file order; bank: array position),
    /// stable under skipping of records that are not dispatched (re-audit RA minor N-m2). Hand-built: push order.
    pub source_index: usize,
    /// 1-based line number in the source file (0 when the source has no lines: bank array, hand-built).
    pub line_no: usize,
    pub witness: WitnessStatus,
    /// What `witness` rests on (`CheckDisabled` when the caller turned the check off).
    pub witness_basis: Assurance,
    /// What the record's target (rule point map) rests on.
    pub target_basis: Assurance,
}

impl RecordMeta {
    /// The verdict that blocks a selected record because of its witness status, if any.
    pub fn witness_block(&self) -> Option<Verdict> {
        if self.witness_basis == Assurance::CheckDisabled {
            return None;
        }
        match self.witness {
            WitnessStatus::Complete => None,
            WitnessStatus::Elided => Some(Verdict::Elided),
            WitnessStatus::NotCarried => Some(Verdict::WitnessNotCarried),
        }
    }
}

/// Which records an attestation covers. Records are named by their `index` field (the engine's rule index: the
/// identifier a raw `table_dump.v1` table and its case-domain export share).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum AttestedRecords {
    #[default]
    None,
    All,
    Indices(BTreeSet<i64>),
}

impl AttestedRecords {
    pub fn covers(&self, index: Option<i64>) -> bool {
        match self {
            AttestedRecords::None => false,
            AttestedRecords::All => true,
            AttestedRecords::Indices(s) => index.is_some_and(|i| s.contains(&i)),
        }
    }
    fn count(&self, tf: &TableFile) -> usize {
        tf.rules.iter().filter(|r| self.covers(r.index)).count()
    }
}

/// A caller's WITNESS / TARGET ATTESTATION for one table whose source format cannot show the property (case-domain
/// exports carry neither witness loci nor targets), given as DATA (re-audit RA-MAJOR-1/2). Passed to
/// [`Table::from_table_file_attested`]. When the raw table the export was derived from is given alongside, the loader
/// CHECKS every covered record against it (same canonical domain; witness: the raw loci are present and not elided,
/// and the raw witness poles are carried over into the domain; target: the raw target is canonical) and a false
/// claim is a load error; otherwise the attestation is recorded as "attested, unchecked" in the provenance and in
/// every credit ([`Assurance::AttestedUnchecked`]).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Attestation {
    /// Records whose witness loci are complete (present, not elided) in the raw source.
    pub witness_complete: AttestedRecords,
    /// Records whose target is the canonical pattern of their case.
    pub targets_canonical: AttestedRecords,
    /// Digest of the raw table the export was derived from (recorded verbatim; the record-by-record check is what
    /// is checked, not the digest).
    pub source_digest: Option<String>,
    /// Provenance text (who attests, from what).
    pub provenance: String,
}

impl Attestation {
    /// The attestation a raw `table_dump.v1` table supports: witness complete for every record whose `witness_loci`
    /// are present and not elided, targets canonical for every record whose target is canonical. Records without
    /// `index` cannot be named and are left out.
    pub fn from_raw_table(
        raw: &TableFile,
        source_digest: Option<String>,
        provenance: impl Into<String>,
    ) -> Result<Attestation, LoadError> {
        let h = raw.header.as_ref().ok_or(LoadError::NoHeader)?;
        let frame = NodeFrame::from_header(h).map_err(LoadError::Frame)?;
        let (mut w, mut t) = (BTreeSet::new(), BTreeSet::new());
        for r in &raw.rules {
            let Some(i) = r.index else { continue };
            if r.witness_loci
                .as_ref()
                .is_some_and(|w| !w.iter().any(|s| s.trim() == "elided"))
            {
                w.insert(i);
            }
            if let Some(text) = &r.target {
                let fixed = fixed_of(r, frame.arity()).map_err(|err| LoadError::Record {
                    line_no: r.line_no,
                    err,
                })?;
                let slots = parse_target(text, frame.arity()).map_err(|e| LoadError::Record {
                    line_no: r.line_no,
                    err: DomainError::Target(e),
                })?;
                if is_canonical_target(&slots, &fixed) {
                    t.insert(i);
                }
            }
        }
        Ok(Attestation {
            witness_complete: AttestedRecords::Indices(w),
            targets_canonical: AttestedRecords::Indices(t),
            source_digest,
            provenance: provenance.into(),
        })
    }
}

/// How an attestation was used while loading a table (in [`TableProvenance::attestation`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttestationRecord {
    pub provenance: String,
    pub source_digest: Option<String>,
    /// True iff every covered record was checked against the raw table.
    pub checked: bool,
    /// Records of the table covered by the witness / target claim.
    pub witness_records: usize,
    pub target_records: usize,
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
    /// The witness / target attestation used, if any ([`Table::from_table_file_attested`]).
    pub attestation: Option<AttestationRecord>,
}

impl TableProvenance {
    fn manual(name: &str) -> Self {
        TableProvenance {
            source: name.to_string(),
            format: SourceFormat::Manual,
            targets: None,
            witness: WitnessProvenance::NotApplicable,
            skipped_not_dispatched: vec![],
            attestation: None,
        }
    }
}

/// Stride of the packed labels of an eps table ([`eps_label`]).
pub const EPS_LABEL_STRIDE: i64 = 1 << 32;

/// The label of the record with engine `index` in chart number `chart_ordinal` (position in
/// [`NodeFrame::charts`]) of an eps table: `chart_ordinal * 2^32 + index`; `None` if `index` is outside `0..2^32`.
pub fn eps_label(chart_ordinal: usize, index: i64) -> Option<i64> {
    let ord = i64::try_from(chart_ordinal).ok()?;
    ((0..EPS_LABEL_STRIDE).contains(&index))
        .then(|| ord.checked_mul(EPS_LABEL_STRIDE)?.checked_add(index))
        .flatten()
}

/// A named rule table.
#[derive(Clone, Debug)]
pub struct Table {
    pub name: String,
    arity: usize,
    pub domains: Vec<CaseDomain>,
    /// One label per domain (record `i`, `index` or bank id), unique per table. In a table of an eps-shifted family
    /// the engine restarts `index` in every chart, so the label is the PACKED pair (chart ordinal in
    /// [`NodeFrame::charts`], index): [`eps_label`] / [`Table::eps_rule_of_label`]; a label never names a record by
    /// its index alone there.
    pub labels: Vec<i64>,
    /// One entry per domain: source position, witness status and what it rests on (re-audit RA-MAJOR-1, N-m2).
    pub meta: Vec<RecordMeta>,
    pub provenance: TableProvenance,
    /// Eps-shifted family ([`Table::from_table_file_eps_aware`]): the node mask and the eps axes. Its domains carry no
    /// orthant chart; the dispatcher tests the node orthant on the other slots (eps axes are two-sided).
    eps_chart: Option<(Vec<bool>, Vec<usize>)>,
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
    /// Two records of one source with the same label.
    DuplicateLabel {
        line_no: usize,
        label: i64,
    },
    /// An attestation that the raw table contradicts, or that cannot be checked as given (record missing from the
    /// raw table, duplicate `index`, different domain, elided or missing raw witness loci, non-canonical raw target).
    Attestation {
        line_no: usize,
        msg: String,
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
            LoadError::DuplicateLabel { line_no, label } => {
                write!(f, "line {line_no}: duplicate label {label}")
            }
            LoadError::Attestation { line_no, msg } => {
                write!(f, "line {line_no}: attestation: {msg}")
            }
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
            meta: vec![],
            eps_chart: None,
        }
    }
    /// Append a hand-built domain (validated, arity checked). Its [`RecordMeta`]: source index = push position,
    /// witness `Complete` (or `Elided` if the domain says so) and target, both resting on [`Assurance::Caller`]: the
    /// caller built the domain and its loci.
    pub fn push(&mut self, label: i64, d: CaseDomain) -> Result<(), CoverError> {
        let meta = RecordMeta {
            source_index: self.domains.len(),
            line_no: 0,
            witness: if d.witness_elided {
                WitnessStatus::Elided
            } else {
                WitnessStatus::Complete
            },
            witness_basis: Assurance::Caller,
            target_basis: Assurance::Caller,
        };
        self.push_with_meta(label, d, meta)
    }
    /// Append a domain with explicit per-record data (validated, arity checked).
    pub fn push_with_meta(
        &mut self,
        label: i64,
        d: CaseDomain,
        meta: RecordMeta,
    ) -> Result<(), CoverError> {
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
        self.meta.push(meta);
        Ok(())
    }
    pub fn arity(&self) -> usize {
        self.arity
    }
    /// For a table of an eps-shifted family: the (chart mask string, engine rule index) a label names (`None` for an
    /// integer table or a label that is not of this table's packed form).
    pub fn eps_rule_of_label(&self, label: i64) -> Option<(String, i64)> {
        let (mask, axes) = self.eps_chart.as_ref()?;
        let (ord, index) = (
            label.div_euclid(EPS_LABEL_STRIDE),
            label.rem_euclid(EPS_LABEL_STRIDE),
        );
        let mut charts: Vec<Vec<bool>> = (0..1u64 << axes.len())
            .map(|bits| {
                let mut c = mask.clone();
                for (j, &a) in axes.iter().enumerate() {
                    c[a] = bits >> j & 1 == 1;
                }
                c
            })
            .collect();
        charts.sort();
        let c = charts.get(usize::try_from(ord).ok()?)?;
        Some((
            c.iter().map(|&b| if b { '1' } else { '0' }).collect(),
            index,
        ))
    }
    /// The node mask and eps axes of a table of an eps-shifted family (`None` for an integer family).
    pub fn eps_chart(&self) -> Option<(&[bool], &[usize])> {
        self.eps_chart
            .as_ref()
            .map(|(m, a)| (m.as_slice(), a.as_slice()))
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
    /// the records' `i`, else `index` (unique per table). Records with `dispatch` other than `Partition` are recorded
    /// in the provenance and are not candidates. Any other unreadable record is an error (fail closed).
    ///
    /// STRICT witness status (re-audit RA-MAJOR-1): a record without `witness_loci` (every record of a case-domain
    /// export) is [`WitnessStatus::NotCarried`] and blocks every key where it is selected; supply an [`Attestation`]
    /// with [`Table::from_table_file_attested`], or turn the check off with [`DomainOptions::witness`] = false
    /// (visible as [`Assurance::CheckDisabled`] in every credit). Targets stated by the legacy
    /// [`TargetSource::CanonicalByConstruction`] text rest on [`Assurance::AttestedUnchecked`].
    pub fn from_table_file(
        name: impl Into<String>,
        source: impl Into<String>,
        tf: &TableFile,
        opts: DomainOptions,
    ) -> Result<Table, LoadError> {
        Self::load(name.into(), source.into(), tf, opts, None, None, false)
    }

    /// [`Table::from_table_file`] for callers audited for eps-shifted families: the frame is read with
    /// [`NodeFrame::from_header_eps_aware`]; `chart` lines must be charts of the frame ([`NodeFrame::charts`]) with
    /// the engine's guards ([`NodeFrame::chart_guards`]: `two_sided:n>=1|n<=0` on an eps axis); every record's
    /// chart decides its eps-axis signs (see [`crate::domain`]). Recenter bounds are refused on an eps frame. On an
    /// integer family this is exactly [`Table::from_table_file`].
    pub fn from_table_file_eps_aware(
        name: impl Into<String>,
        source: impl Into<String>,
        tf: &TableFile,
        opts: DomainOptions,
    ) -> Result<Table, LoadError> {
        Self::load(name.into(), source.into(), tf, opts, None, None, true)
    }

    /// [`Table::from_table_file`] with a witness / target [`Attestation`] (re-audit RA-MAJOR-1/2). With `raw` (the
    /// raw `table_dump.v1` table the export was derived from) every covered record is CHECKED against the raw record
    /// with the same `index`: same canonical domain, witness loci present and not elided (the raw witness poles are
    /// then carried into the domain), target canonical; any contradiction is [`LoadError::Attestation`]. The covered
    /// records rest on [`Assurance::AttestedChecked`]. Without `raw` they rest on [`Assurance::AttestedUnchecked`].
    /// Records the attestation does not cover are loaded as by [`Table::from_table_file`].
    pub fn from_table_file_attested(
        name: impl Into<String>,
        source: impl Into<String>,
        tf: &TableFile,
        opts: DomainOptions,
        attestation: &Attestation,
        raw: Option<&TableFile>,
    ) -> Result<Table, LoadError> {
        Self::load(
            name.into(),
            source.into(),
            tf,
            opts,
            Some(attestation),
            raw,
            false,
        )
    }

    fn load(
        name: String,
        source: String,
        tf: &TableFile,
        opts: DomainOptions,
        att: Option<&Attestation>,
        raw: Option<&TableFile>,
        eps_aware: bool,
    ) -> Result<Table, LoadError> {
        let h = tf.header.as_ref().ok_or(LoadError::NoHeader)?;
        let frame = if eps_aware {
            NodeFrame::from_header_eps_aware(h)
        } else {
            NodeFrame::from_header(h)
        }
        .map_err(LoadError::Frame)?;
        let mask = frame.mask_string();
        let mut seen_charts = BTreeSet::new();
        for (c, guards) in &tf.charts {
            let bits: Vec<bool> = c.chars().map(|x| x == '1').collect();
            let Some(want) = frame
                .chart_guards(&bits)
                .filter(|_| c.chars().all(|x| x == '0' || x == '1'))
            else {
                return Err(LoadError::Chart(format!(
                    "{c} is not a chart of the node (mask {mask}, eps axes {:?})",
                    frame.eps_axes()
                )));
            };
            if *guards != want {
                return Err(LoadError::Chart(format!("unsupported guards {guards:?}")));
            }
            if !seen_charts.insert(c.clone()) {
                return Err(LoadError::Chart(format!("chart {c} twice")));
            }
        }
        // the raw table of an attestation, indexed by `index`
        let raw_side = match (att, raw) {
            (Some(_), Some(rt)) => {
                let rh = rt.header.as_ref().ok_or(LoadError::NoHeader)?;
                let rframe = NodeFrame::from_header(rh).map_err(LoadError::Frame)?;
                frame
                    .require_integer_only_for("attested loading")
                    .map_err(LoadError::Frame)?;
                if !rframe.same_mask(&frame) || rframe.arity() != frame.arity() {
                    return Err(LoadError::Attestation {
                        line_no: 0,
                        msg: format!(
                            "raw table mask {} differs from {mask}",
                            rframe.mask_string()
                        ),
                    });
                }
                let mut by_index: HashMap<i64, &RuleRecord> = HashMap::new();
                for r in &rt.rules {
                    let Some(i) = r.index else { continue };
                    if by_index.insert(i, r).is_some() {
                        return Err(LoadError::Attestation {
                            line_no: r.line_no,
                            msg: format!("raw table has index {i} twice"),
                        });
                    }
                }
                Some((rframe, by_index))
            }
            _ => None,
        };
        let mut t = Table::new(name, frame.arity());
        if frame.has_eps_axes() {
            t.eps_chart = Some((frame.mask().to_vec(), frame.eps_axes()));
        }
        let fmt = match tf.rules.first().map(|r| r.format) {
            Some(RecordFormat::CaseExport) => SourceFormat::CaseExport,
            _ => SourceFormat::TableDump,
        };
        let with_w = tf.rules.iter().filter(|r| r.witness_loci.is_some()).count();
        t.provenance = TableProvenance {
            source,
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
            attestation: att.map(|a| AttestationRecord {
                provenance: a.provenance.clone(),
                source_digest: a.source_digest.clone(),
                checked: raw_side.is_some(),
                witness_records: a.witness_complete.count(tf),
                target_records: a.targets_canonical.count(tf),
            }),
        };
        let attested_basis = if raw_side.is_some() {
            Assurance::AttestedChecked
        } else {
            Assurance::AttestedUnchecked
        };
        let mut seen_labels = std::collections::HashSet::new();
        let charts = frame.charts();
        for (source_index, r) in tf.rules.iter().enumerate() {
            let mut label =
                r.i.or(r.index)
                    .ok_or(LoadError::NoLabel { line_no: r.line_no })?;
            if frame.has_eps_axes() {
                // the engine restarts `index` in every chart: the label is (chart, index), packed (eps_label)
                let bits: Vec<bool> = r
                    .chart
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .map(|x| x == '1')
                    .collect();
                label = charts
                    .iter()
                    .position(|c| *c == bits)
                    .and_then(|ord| eps_label(ord, label))
                    .ok_or_else(|| LoadError::Record {
                        line_no: r.line_no,
                        err: DomainError::Malformed(format!(
                            "eps table record: chart {:?} not a chart of the frame or index {label} outside 0..2^32",
                            r.chart
                        )),
                    })?;
            }
            let claim_w = att.is_some_and(|a| a.witness_complete.covers(r.index));
            let claim_t =
                att.is_some_and(|a| a.targets_canonical.covers(r.index)) && r.target.is_none();
            let ropts = if claim_t {
                DomainOptions {
                    targets: TargetSource::CanonicalByConstruction {
                        attestation: ATTESTED_TARGETS,
                    },
                    ..opts
                }
            } else {
                opts
            };
            let mut d = match CaseDomain::from_rule_record(r, &frame, ropts) {
                Ok(d) => d,
                Err(DomainError::NotDispatched { dispatch }) => {
                    t.provenance.skipped_not_dispatched.push(SkippedRecord {
                        line_no: r.line_no,
                        label,
                        dispatch,
                    });
                    continue;
                }
                Err(err) => {
                    return Err(LoadError::Record {
                        line_no: r.line_no,
                        err,
                    });
                }
            };
            if !seen_labels.insert(label) {
                return Err(LoadError::DuplicateLabel {
                    line_no: r.line_no,
                    label,
                });
            }
            let mut witness = match &r.witness_loci {
                None => WitnessStatus::NotCarried,
                Some(w) if w.iter().any(|s| s.trim() == "elided") => WitnessStatus::Elided,
                Some(_) => WitnessStatus::Complete,
            };
            let mut witness_basis = Assurance::Record;
            let mut target_basis = if r.target.is_some() {
                Assurance::Record
            } else {
                Assurance::AttestedUnchecked
            };
            if (claim_w && r.witness_loci.is_none()) || claim_t {
                if let Some((rframe, by_index)) = &raw_side {
                    check_against_raw(r, &d, rframe, by_index, claim_w, claim_t, opts)?
                        .into_iter()
                        .for_each(|p| d.poles.push(p));
                }
                if claim_w && r.witness_loci.is_none() {
                    witness = WitnessStatus::Complete;
                    witness_basis = attested_basis;
                }
                if claim_t {
                    target_basis = attested_basis;
                }
            }
            if !opts.witness {
                witness_basis = Assurance::CheckDisabled;
            }
            let meta = RecordMeta {
                source_index,
                line_no: r.line_no,
                witness,
                witness_basis,
                target_basis,
            };
            t.push_with_meta(label, d, meta)
                .map_err(|e| LoadError::Record {
                    line_no: r.line_no,
                    err: DomainError::Invalid(e.to_string()),
                })?;
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
            attestation: None,
        };
        let mut seen = std::collections::HashSet::new();
        for (source_index, p) in preds.iter().enumerate() {
            let d = CaseDomain::from_bank_predicate(p, arity, targets)
                .map_err(|err| LoadError::Record { line_no: 0, err })?;
            if !seen.insert(p.global_ordinal0) {
                return Err(LoadError::DuplicateLabel {
                    line_no: 0,
                    label: p.global_ordinal0,
                });
            }
            // the bank carries its witness loci (`raw_rule_witness_loci_generic_d_nonzero`, a required list): status
            // from the data; the target rests on the caller's CanonicalByConstruction statement (unchecked)
            let meta = RecordMeta {
                source_index,
                line_no: 0,
                witness: if d.witness_elided {
                    WitnessStatus::Elided
                } else if p.witness_loci.is_some() {
                    WitnessStatus::Complete
                } else {
                    WitnessStatus::NotCarried
                },
                witness_basis: Assurance::Record,
                target_basis: Assurance::AttestedUnchecked,
            };
            t.push_with_meta(p.global_ordinal0, d, meta)
                .map_err(|e| LoadError::Record {
                    line_no: 0,
                    err: DomainError::Invalid(e.to_string()),
                })?;
        }
        Ok(t)
    }
}

/// The provenance text recorded as the target statement of attested records.
const ATTESTED_TARGETS: &str = "attestation (see TableProvenance::attestation)";

/// Check one export record against the raw record with the same `index` (the attestation claims witness and/or
/// target). Returns the raw record's witness pole loci (to carry into the export's domain) when the witness claim is
/// checked.
fn check_against_raw(
    r: &RuleRecord,
    d: &CaseDomain,
    rframe: &NodeFrame,
    by_index: &HashMap<i64, &RuleRecord>,
    claim_w: bool,
    claim_t: bool,
    opts: DomainOptions,
) -> Result<Vec<crate::domain::PoleLocus>, LoadError> {
    let bad = |msg: String| LoadError::Attestation {
        line_no: r.line_no,
        msg,
    };
    let idx = r
        .index
        .ok_or_else(|| bad("record without index cannot be checked".into()))?;
    let rr = by_index
        .get(&idx)
        .ok_or_else(|| bad(format!("index {idx} is not in the raw table")))?;
    if rr.target.is_none() {
        return Err(bad(format!("raw record {idx} carries no target")));
    }
    let plain = DomainOptions {
        targets: TargetSource::FromRecord,
        witness: false,
        ..opts
    };
    let rd = CaseDomain::from_rule_record(rr, rframe, plain)
        .map_err(|e| bad(format!("raw record {idx}: {e}")))?;
    if claim_t && rd.target != TargetMap::Canonical {
        return Err(bad(format!("raw record {idx}: target is not canonical")));
    }
    let mine = CaseDomain {
        poles: vec![],
        witness_elided: false,
        ..d.clone()
    };
    if mine.canonical_form() != rd.canonical_form() {
        return Err(bad(format!(
            "raw record {idx}: domain differs from the export's"
        )));
    }
    if !claim_w {
        return Ok(vec![]);
    }
    match &rr.witness_loci {
        None => Err(bad(format!("raw record {idx}: no witness_loci"))),
        Some(w) if w.iter().any(|s| s.trim() == "elided") => {
            Err(bad(format!("raw record {idx}: witness loci elided")))
        }
        Some(_) if !opts.witness => Ok(vec![]),
        Some(_) => {
            let rw = CaseDomain::from_rule_record(
                rr,
                rframe,
                DomainOptions {
                    witness: true,
                    ..plain
                },
            )
            .map_err(|e| bad(format!("raw record {idx}: {e}")))?;
            Ok(rw.poles)
        }
    }
}

/// A credited (or blocking) rule: table (index into the dispatcher's table list), the record's DISPATCH position in
/// the table (`record`, an index into [`Table::domains`]), its position in the SOURCE file (`source_index`, stable
/// under skipped records: credits name the rule by it; re-audit RA minor N-m2), its label, and the basis of the
/// credit ([`CreditBasis`]: what its witness status and target rest on, whether the chart was tested).
///
/// Equality and hash are the RULE IDENTITY `(table, record, label)`: `source_index` and `basis` are functions of the
/// dispatcher's tables. Build one with [`Dispatcher::rule_ref`] (full data) or [`RuleRef::bare`] (identity only).
#[derive(Clone, Copy, Debug)]
pub struct RuleRef {
    pub table: usize,
    /// Position in [`Table::domains`] (dispatch position; equals `source_index` when no record was skipped).
    pub record: usize,
    /// Position among the rule records of the source ([`RecordMeta::source_index`]).
    pub source_index: usize,
    pub label: i64,
    pub basis: CreditBasis,
}

impl PartialEq for RuleRef {
    fn eq(&self, o: &Self) -> bool {
        (self.table, self.record, self.label) == (o.table, o.record, o.label)
    }
}
impl Eq for RuleRef {}
impl std::hash::Hash for RuleRef {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        (self.table, self.record, self.label).hash(h)
    }
}

impl RuleRef {
    /// An identity-only reference for local indexing (lookups by position, hand-built domain lists): `source_index`
    /// = `record`, basis `Caller` / `Caller`, chart not checked. Never use it as a credit of a dispatcher: take
    /// [`Dispatcher::rule_ref`].
    pub fn bare(table: usize, record: usize, label: i64) -> RuleRef {
        RuleRef {
            table,
            record,
            source_index: record,
            label,
            basis: CreditBasis {
                witness: Assurance::Caller,
                target: Assurance::Caller,
                chart_checked: false,
            },
        }
    }
}

/// Union mode (legacy tuple of [`Dispatcher::union_cover`]): the applying rules, and the rules selected but blocked or
/// undecided (with their verdicts).
pub type UnionCover = (Vec<RuleRef>, Vec<(RuleRef, Verdict)>);

/// Union mode ([`Dispatcher::union`]): a key outside the common chart is told apart from a key no rule applies to
/// (re-audit RA minor N-m3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnionResult {
    /// The key is outside the common chart (first violating slot): no rule is consulted.
    OutsideChart { slot: usize },
    /// Every applying rule (`applies`, priority order) and every rule selected but blocked or undecided (`open`,
    /// with its verdict; a record whose witness status is not `Complete` is here with its witness verdict).
    Rules {
        applies: Vec<RuleRef>,
        open: Vec<(RuleRef, Verdict)>,
    },
}

impl UnionResult {
    /// True iff at least one rule applies (union coverage).
    pub fn is_covered(&self) -> bool {
        matches!(self, UnionResult::Rules { applies, .. } if !applies.is_empty())
    }
}

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
    /// [`Dispatcher::union_cover`] (legacy) at a key outside the common chart: the old empty answer looked like
    /// "no rule applies" (re-audit RA minor N-m3); use [`Dispatcher::union`].
    OutsideChart { slot: usize },
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
            CoverError::OutsideChart { slot } => {
                write!(f, "key outside the common chart (slot {slot})")
            }
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
    /// The common chart of the domains that carry one (or given with [`Dispatcher::with_chart`]).
    chart: Option<Vec<bool>>,
    /// Eps axes of an eps-shifted family: two-sided slots the node-orthant test of `chart` skips.
    chart_free: Vec<usize>,
    /// Per entry: the verdict that blocks the record when it is selected because of its witness status.
    entry_block: Vec<Option<Verdict>>,
}

/// Counts of the credits of a batch by their basis (re-audit RA-MAJOR-1/2: a non-default policy or an unchecked
/// attestation is never silent).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreditTally {
    /// Credits whose witness or target rests on an unchecked attestation ([`Assurance::AttestedUnchecked`]).
    pub attested_unchecked: usize,
    /// Credits whose witness or target rests on a checked attestation (and nothing unchecked).
    pub attested_checked: usize,
    /// Credits with the witness check turned off ([`Assurance::CheckDisabled`]).
    pub witness_check_off: usize,
    /// Credits from hand-built domains ([`Assurance::Caller`]).
    pub caller: usize,
    /// Credits given without a chart test.
    pub chart_unchecked: usize,
    /// Credits on the strict basis ([`CreditBasis::is_strict`]).
    pub strict: usize,
    /// Blocked keys whose reason is the witness status ([`Verdict::Elided`] or [`Verdict::WitnessNotCarried`]).
    pub blocked_witness: usize,
}

impl CreditTally {
    fn add_credit(&mut self, b: &CreditBasis) {
        if b.rests_on_unchecked_attestation() {
            self.attested_unchecked += 1;
        } else if b.witness == Assurance::AttestedChecked || b.target == Assurance::AttestedChecked
        {
            self.attested_checked += 1;
        }
        if b.witness == Assurance::CheckDisabled {
            self.witness_check_off += 1;
        }
        if b.witness == Assurance::Caller || b.target == Assurance::Caller {
            self.caller += 1;
        }
        if !b.chart_checked {
            self.chart_unchecked += 1;
        }
        if b.is_strict() {
            self.strict += 1;
        }
    }
}

/// Per table: how many records rest on what (see [`Dispatcher::summary`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableAssurance {
    pub name: String,
    pub records: usize,
    /// Records whose witness status is not `Complete` (they block where selected, unless the check is off).
    pub witness_not_complete: usize,
    pub witness_check_off: usize,
    pub witness_attested_unchecked: usize,
    pub witness_attested_checked: usize,
    pub target_attested_unchecked: usize,
    pub target_attested_checked: usize,
    pub caller: usize,
}

/// What the dispatcher's credits rest on, table by table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DispatchSummary {
    pub chart_checked: bool,
    pub tables: Vec<TableAssurance>,
}

impl DispatchSummary {
    /// No witness check off, no unchecked attestation, no hand-built record, chart tested.
    pub fn is_strict(&self) -> bool {
        self.chart_checked
            && self.tables.iter().all(|t| {
                t.witness_check_off == 0
                    && t.witness_attested_unchecked == 0
                    && t.target_attested_unchecked == 0
                    && t.caller == 0
            })
    }
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
    /// The credits by basis and the witness-blocked keys.
    pub tally: CreditTally,
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
        let mut eps_chart: Option<&(Vec<bool>, Vec<usize>)> = None;
        for t in &tables {
            match (&t.eps_chart, eps_chart) {
                (Some(e), None) => eps_chart = Some(e),
                (Some(e), Some(e0)) if e != e0 => {
                    return Err(CoverError::Invalid(format!(
                        "table {}: eps axes / mask differ from the other tables'",
                        t.name
                    )));
                }
                _ => {}
            }
            if t.arity != arity {
                return Err(CoverError::Arity(format!(
                    "table {} of arity {}, expected {arity}",
                    t.name, t.arity
                )));
            }
            if t.labels.len() != t.domains.len() || t.meta.len() != t.domains.len() {
                return Err(CoverError::Invalid(format!(
                    "table {}: {} labels and {} record entries for {} domains",
                    t.name,
                    t.labels.len(),
                    t.meta.len(),
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
        let mut chart_free = vec![];
        if let Some((mask, axes)) = eps_chart {
            if chart.is_some() || tables.iter().any(|t| t.eps_chart.is_none()) {
                return Err(CoverError::Invalid(
                    "a table of an eps-shifted family together with integer-chart tables".into(),
                ));
            }
            chart = Some(mask.clone());
            chart_free = axes.clone();
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
        let entry_block = entries
            .iter()
            .map(|&(t, r)| tables[t as usize].meta[r as usize].witness_block())
            .collect();
        Ok(Dispatcher {
            tables,
            priority,
            entries,
            patterns,
            arity,
            chart,
            chart_free,
            entry_block,
        })
    }

    /// Test every key against `mask` (line slots >= 1, other slots <= 0) before dispatch: a key outside it is
    /// [`Dispatch::OutsideChart`]. For dispatchers whose domains carry no chart (bank predicates, `chart: false`):
    /// without it their credits say `chart_checked: false` (re-audit RA minor N-m4). The mask must have the
    /// dispatcher's arity and agree with the domains' common chart, if any.
    pub fn with_chart(mut self, mask: Vec<bool>) -> Result<Self, CoverError> {
        if mask.len() != self.arity {
            return Err(CoverError::Arity(format!(
                "chart of length {} for arity {}",
                mask.len(),
                self.arity
            )));
        }
        if let Some(c) = &self.chart
            && *c != mask
        {
            return Err(CoverError::Invalid(
                "chart differs from the domains' common chart".into(),
            ));
        }
        self.chart = Some(mask);
        Ok(self)
    }

    /// What the credits of this dispatcher rest on, table by table (re-audit RA-MAJOR-1/2).
    pub fn summary(&self) -> DispatchSummary {
        DispatchSummary {
            chart_checked: self.chart.is_some(),
            tables: self
                .tables
                .iter()
                .map(|t| {
                    let c =
                        |f: &dyn Fn(&RecordMeta) -> bool| t.meta.iter().filter(|m| f(m)).count();
                    TableAssurance {
                        name: t.name.clone(),
                        records: t.meta.len(),
                        witness_not_complete: c(&|m| m.witness != WitnessStatus::Complete),
                        witness_check_off: c(&|m| m.witness_basis == Assurance::CheckDisabled),
                        witness_attested_unchecked: c(&|m| {
                            m.witness_basis == Assurance::AttestedUnchecked
                        }),
                        witness_attested_checked: c(&|m| {
                            m.witness_basis == Assurance::AttestedChecked
                        }),
                        target_attested_unchecked: c(&|m| {
                            m.target_basis == Assurance::AttestedUnchecked
                        }),
                        target_attested_checked: c(&|m| {
                            m.target_basis == Assurance::AttestedChecked
                        }),
                        caller: c(&|m| {
                            m.witness_basis == Assurance::Caller
                                || m.target_basis == Assurance::Caller
                        }),
                    }
                })
                .collect(),
        }
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
        key.iter().enumerate().position(|(i, &x)| {
            !self.chart_free.contains(&i) && ((mask[i] && x < 1) || (!mask[i] && x > 0))
        })
    }
    /// Eps axes of an eps-shifted family (two-sided slots the chart test skips); empty for an integer family.
    pub fn chart_free(&self) -> &[usize] {
        &self.chart_free
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
        self.rule_ref(t as usize, r as usize)
    }

    /// The full reference (source index, label, credit basis) of record `record` (dispatch position) of table
    /// `table`. For modules that run their own dispatch over [`Dispatcher::tables`] (closure, partition): their credits
    /// then carry the same basis as [`Dispatcher::first_match`]'s. Panics on an index out of range.
    pub fn rule_ref(&self, table: usize, record: usize) -> RuleRef {
        let (t, r) = (table, record);
        let tb = &self.tables[t];
        let m = &tb.meta[r];
        RuleRef {
            table: t,
            record: r,
            source_index: m.source_index,
            label: tb.labels[r],
            basis: CreditBasis {
                witness: m.witness_basis,
                target: m.target_basis,
                chart_checked: self.chart.is_some(),
            },
        }
    }

    /// The domain verdict, with a selected record whose witness status is not `Complete` turned into its witness
    /// verdict (re-audit RA-MAJOR-1).
    fn enforced_verdict(&self, e: u32, key: &[i32]) -> Verdict {
        let v = self.verdict(e, key);
        match (&v, &self.entry_block[e as usize]) {
            (Verdict::Applies, Some(b)) => b.clone(),
            _ => v,
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
            let v = self.enforced_verdict(e, key);
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

    /// UNION coverage (not dispatch): every rule that applies at `key`, in priority order, and the rules that are
    /// selected but blocked or undecided at the key, with their verdicts (witness status enforced as in first match).
    /// A key outside the common chart is [`UnionResult::OutsideChart`] (re-audit RA minor N-m3).
    pub fn union(&self, key: &[i32]) -> Result<UnionResult, CoverError> {
        self.check_key(key)?;
        if let Some(slot) = self.outside_chart(key) {
            return Ok(UnionResult::OutsideChart { slot });
        }
        let mut c = Vec::new();
        let (mut applies, mut open) = (vec![], vec![]);
        self.candidates(key, &mut c);
        for &e in &c {
            let v = self.enforced_verdict(e, key);
            match v.stage() {
                Stage::Applies => applies.push(self.rref(e)),
                Stage::NotSelected => {}
                Stage::Blocked | Stage::Undecided => open.push((self.rref(e), v)),
            }
        }
        Ok(UnionResult::Rules { applies, open })
    }

    /// LEGACY union coverage as a tuple ([`UnionCover`]). STRICT: a key outside the common chart is the error
    /// [`CoverError::OutsideChart`] (it used to be `([], [])`, which looked like "no rule applies").
    #[deprecated(note = "use Dispatcher::union (UnionResult tells outside-chart from uncovered)")]
    pub fn union_cover(&self, key: &[i32]) -> Result<UnionCover, CoverError> {
        match self.union(key)? {
            UnionResult::OutsideChart { slot } => Err(CoverError::OutsideChart { slot }),
            UnionResult::Rules { applies, open } => Ok((applies, open)),
        }
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
            tally: CreditTally::default(),
            seconds: 0.0,
        };
        for r in &results {
            match r {
                Dispatch::Covered(w) => {
                    rep.wins[w.table] += 1;
                    rep.covered += 1;
                    rep.tally.add_credit(&w.basis);
                }
                Dispatch::Uncovered => rep.uncovered += 1,
                Dispatch::Blocked { reason, .. } => {
                    rep.blocked += 1;
                    if matches!(reason, Verdict::Elided | Verdict::WitnessNotCarried) {
                        rep.tally.blocked_witness += 1;
                    }
                }
                Dispatch::Undecided { .. } => rep.undecided += 1,
                Dispatch::OutsideChart { .. } => rep.outside_chart += 1,
            }
        }
        rep.results = results;
        rep.seconds = t0.elapsed().as_secs_f64();
        Ok(rep)
    }
}
