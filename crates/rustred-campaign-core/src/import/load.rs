//! `load`: a [`TableSetSpec`] -> the in-force dispatch ([`ImportedSet`]) with evidence level `imported`.
//!
//! Every source becomes one or more core dispatcher tables (in spec order) or a relation set (recipes, supplied point
//! relations) or declared terminals / a credit policy. Every dispatched record keeps its provenance ([`RuleInfo`]:
//! source, file, ordinal, byte offset, `record_sha256`, hand name). Refusals are typed ([`ImportError`]); nothing is
//! dropped silently (records the engine does not dispatch are listed in the report).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use serde_json::Value;

use super::error::{ImportError, io_err};
use super::index::{IndexOrigin, TableIndex, open_scan_checked};
use super::lines::{
    MAX_BOUND_EXCLUSIONS_PER_SLOT, MarkerPolicy, RawRecord, RawTable, SymfaceRef, check_markers,
    parse_key_text, read_sha256sums, scan_table_with, verify_file,
};
use super::spec::{
    DomainMode, HandName, SourceKind, SourceSpec, StartFormat, TableSetSpec, TableTerminals,
    resolve,
};
use super::symmetry::{
    DEFAULT_MAX_TERMS, GroupFormat, GroupReport, GroupSpec, RelationCheck, load_verified_group,
    load_verified_group_family, verify_relation_with, verify_sym_record,
};
use crate::closure::relations::{
    DispatchSlot, RelationSet, RelationStoreBuilder, VERDICT_DESCENDS, VERDICT_EXACT,
    VERDICT_REGULAR,
};
use crate::closure::{KeyCodec, KeySet, KeySetBuilder, RuleBodies};
use crate::cover::{Dispatcher, Table, WitnessStatus};
use crate::domain::{CaseDomain, DomainOptions, TargetMap};
use crate::domain::{PoleKind, PoleLocus};
use crate::key::{FrameError, NodeFrame};
use crate::order::{GroupElement, NodeComparator};
use crate::poly::Poly;
use crate::record::{TableFile, TableHeader};
use crate::replay_base::family::Family;
use crate::replay_base::symrows::VerifiedGroup;
use crate::symmetry::CreditPolicy;

/// Witness status of an imported record as the dispatch sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessKind {
    Complete,
    NotCarried,
    Elided,
}

/// Provenance of one dispatched record.
#[derive(Clone, Debug, Serialize)]
pub struct RuleInfo {
    pub source: String,
    pub class: String,
    pub table_name: String,
    pub path: String,
    pub ordinal: usize,
    pub label: i64,
    pub index: Option<i64>,
    pub offset: u64,
    pub bytes: u64,
    pub record_sha256: String,
    pub hand_name: Option<String>,
    pub witness: WitnessKind,
    /// A `sym` record (one-term symmetry rule; interfaces/import.md 18.4): its verification. None for rule records.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sym: Option<SymInfo>,
    /// The `symface` provenance of a slice record (18.5). None otherwise.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symface: Option<SymfaceRef>,
}

/// The verification of one `sym` record (interfaces/import.md 18.4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SymInfo {
    /// Verified by the seven steps against our derived group: serves (basis `sym_record_verified`); else every key
    /// of its case is blocked (`sym_record_unverified: <reason>`).
    pub verified: bool,
    /// The failed step (unverified only).
    pub reason: Option<String>,
    /// The element of our derived group with the same action (verified only).
    pub element: Option<usize>,
    /// The record's action (`(target slot, flip)` per slot; None = fixed at 0 on the case): the image of a key.
    #[serde(skip)]
    pub action: Vec<Option<(usize, bool)>>,
}

impl SymInfo {
    /// The image `g a` of a key `a` of the case (slot `t` carries the power of the slot `i` with action `(t, _)`).
    pub fn image_of(&self, key: &[i32]) -> Vec<i32> {
        let mut out = vec![0; key.len()];
        for (i, a) in self.action.iter().enumerate() {
            if let Some((t, _)) = a
                && *t < out.len()
                && i < key.len()
            {
                out[*t] = key[i];
            }
        }
        out
    }
}

/// Where a TABLE-declared terminal comes from (interfaces/import.md 18.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TerminalDecl {
    /// `composite_terminal_ledger` | `residual`.
    pub provenance: &'static str,
    pub source: String,
    pub path: String,
    pub line: usize,
}

/// `sym` records of a source (interfaces/import.md 18.4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SymReport {
    pub records: usize,
    pub verified: usize,
    /// (label, reason) of every unverified record.
    pub unverified: Vec<(i64, String)>,
    /// Why no verified group could be built (then every sym record of the source is unverified).
    pub group_error: Option<String>,
}

/// A recipe source (its relation set is built by `check` from the certified solved relations).
#[derive(Clone, Debug)]
pub struct RecipeSource {
    pub id: String,
    pub class: String,
    pub path: String,
    pub recipe_sha256: String,
    pub engine_digest: Option<String>,
    pub recipe_id: Option<String>,
    pub domain: CaseDomain,
    pub slot: DispatchSlot,
    /// Per-key assignment (recipe v1.2 `domain.keys` and / or the spec's side file): the ONLY keys this recipe may
    /// serve and be asked for; `None` = its whole domain.
    pub assigned: Option<std::collections::HashSet<Vec<i32>>>,
}

/// A supplied point relation (provenance for statements).
#[derive(Clone, Debug, Serialize)]
pub struct SuppliedInfo {
    pub source: String,
    pub class: String,
    pub path: String,
    pub label: Option<i64>,
    pub ordinal: usize,
    pub record_sha256: String,
    pub hand_name: Option<String>,
    pub verification_sha256: Option<String>,
    /// `verify`: the re-derivation from the verified group at the key (None: not asked).
    pub check: Option<RelationCheck>,
    /// From a `diagnostic` (keys-only) source: followed, never certified.
    pub diagnostic: bool,
}

/// One relation-set entry of the dispatch (in spec order).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelEntry {
    Supplied(usize),
    Recipe(usize),
}

/// What the loader saw, per source (written into statements).
#[derive(Clone, Debug, Default, Serialize)]
pub struct SourceReport {
    pub id: String,
    pub kind: String,
    pub class: String,
    pub files: Vec<(String, String)>,
    pub tables: Vec<String>,
    pub records_in_files: usize,
    pub records_dispatched: usize,
    pub not_dispatched: Vec<(String, i64, String)>,
    pub removed: Vec<(i64, String)>,
    pub witness: BTreeMap<String, usize>,
    pub bounds_records: usize,
    pub relations: usize,
    /// Non-rule line kinds over the source's files (e.g. `chart_error` records of the engine).
    pub line_kinds: BTreeMap<String, usize>,
    /// Files without an `end` record (an unterminated run table: imported as it is, reported).
    pub files_without_end: Vec<String>,
    /// The verified symmetry group of a `symmetry_credit` / `supplied_relations.verify` source.
    pub group: Option<GroupReport>,
    /// `supplied_relations.verify`: relations verified / not verified by re-derivation from the verified group.
    pub relations_verified: usize,
    pub relations_not_verified: Vec<(Vec<i32>, String)>,
    /// Table sources: how each file's index was obtained (one pass over the table, or a reused index; NOT
    /// identifying: a reused index carries the file sha256 recorded when it was built, see `import::index`). Never
    /// serialized into the canonical statement (it holds timings); callers put it into their timing record.
    #[serde(skip_serializing)]
    pub indexes: Vec<IndexOrigin>,
    /// lane import5 (18.3): counts of the marker kinds over the source's files (only when present).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub markers: BTreeMap<String, usize>,
    /// (file, chart) of every chart line without a guard list, read as `chart_guards: implied_by_mask` (the guards
    /// of its mask; engine lane verdict 2026-11-15T03:22Z). Only when present.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub chart_guards_implied: Vec<(String, String)>,
    /// Files that carry a composite-assembly record.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub assembly_files: Vec<String>,
    /// Accepted marker overrides (`marker_rules.accept_fewer`), with the reason.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub marker_overrides: Vec<String>,
    /// Terminals declared by the source's tables (ledger keys; residual keys under `ledger_and_residuals`).
    #[serde(skip_serializing_if = "is_zero")]
    pub table_terminals: usize,
    /// `sym` records (18.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sym: Option<SymReport>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Options of [`load_with`] (not part of the spec: they never change a result).
#[derive(Clone, Debug, Default)]
pub struct LoadOptions {
    /// Directory of the persistent table indexes (`import::index`): an index is reused when the table's path, size,
    /// mtime and sampled digest match; `None` = a private temporary index per table file (one pass all the same).
    pub index_dir: Option<PathBuf>,
}

/// The load report.
#[derive(Clone, Debug, Default, Serialize)]
pub struct LoadReport {
    pub spec_digest: String,
    pub node: Option<String>,
    pub arity: usize,
    pub mask: String,
    pub comparator: String,
    pub start_keys: usize,
    pub start_digest: String,
    pub sources: Vec<SourceReport>,
    pub tables: usize,
    pub records: usize,
    pub relation_sets: usize,
    pub terminals_preferred: usize,
    pub terminals_other: usize,
    /// Of `terminals_other`: declared by tables (18.2).
    #[serde(skip_serializing_if = "is_zero")]
    pub terminals_from_tables: usize,
}

/// The imported set: the in-force dispatch at evidence level `imported`.
pub struct ImportedSet {
    pub spec: TableSetSpec,
    pub frame: NodeFrame,
    pub cmp: NodeComparator,
    pub codec: KeyCodec,
    pub disp: Dispatcher,
    pub bodies: RuleBodies,
    /// `[table][record]` provenance, parallel to the dispatcher's tables.
    pub rules: Vec<Vec<RuleInfo>>,
    /// Table name -> file path (fact identity for the producer) and -> `sha256:` of the file.
    pub fact_tables: BTreeMap<String, String>,
    pub table_digests: BTreeMap<String, String>,
    /// Supplied relation sets (complete at load).
    pub supplied: Vec<Arc<RelationSet>>,
    /// Supplied relation provenance: set name -> key -> info.
    pub supplied_info: BTreeMap<String, BTreeMap<Vec<i32>, SuppliedInfo>>,
    pub recipes: Vec<RecipeSource>,
    /// Relation-set entries in SPEC order (supplied index / recipe index): the dispatch order among relation sets.
    pub rel_order: Vec<RelEntry>,
    pub terminals_preferred: Vec<Vec<i32>>,
    pub terminals_other: Vec<Vec<i32>>,
    pub credits: Option<CreditPolicy>,
    pub start: KeySet,
    /// record_sha256 -> (table, record).
    pub by_sha: HashMap<String, (usize, usize)>,
    /// Source id -> class.
    pub classes: BTreeMap<String, String>,
    pub report: LoadReport,
    /// Under the DIAGNOSTIC `witness_check: off_diagnostic` only: the STRICT domain (witness loci checked) of every
    /// record whose witness loci are complete, by (table, record): what a strict run would decide at a key the
    /// diagnostic dispatch serves (a key on a witness pole is blocked there). Empty otherwise.
    pub strict_domains: HashMap<(usize, usize), CaseDomain>,
    /// lane closeloop4 (audit-closeloop m4): the family of the set's tables (every table header agrees, `check_header`):
    /// spec evidence declared for another family is refused.
    pub family: Option<String>,
    /// lane import5 (18.2): terminals declared by a TABLE (ledger keys, residual keys under `ledger_and_residuals`), with
    /// their provenance. A key declared by a `terminals` source is not listed (first declaration wins).
    pub terminal_provenance: BTreeMap<Vec<i32>, TerminalDecl>,
}

impl std::fmt::Debug for ImportedSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImportedSet")
            .field("report", &self.report)
            .finish()
    }
}

impl ImportedSet {
    pub fn report(&self) -> &LoadReport {
        &self.report
    }
    pub fn rule(&self, table: usize, record: usize) -> &RuleInfo {
        &self.rules[table][record]
    }
    pub fn arity(&self) -> usize {
        self.frame.arity()
    }
    /// The (table, record) of the record with this sha256, if dispatched.
    pub fn find_sha(&self, sha: &str) -> Option<(usize, usize)> {
        self.by_sha.get(sha).copied()
    }
    /// The (table, record) of a dispatcher table name and record position.
    pub fn table_index(&self, name: &str) -> Option<usize> {
        self.disp.tables().iter().position(|t| t.name == name)
    }
    /// The `symface` slice records grouped by face (lane import5, interfaces/import.md 18.5): (the member `face` as
    /// canonical JSON, `element.index`) -> [(table, record)] in dispatch order. Nothing is interpreted.
    pub fn symface_groups(&self) -> BTreeMap<SymfaceGroup, Vec<(usize, usize)>> {
        let mut m: BTreeMap<SymfaceGroup, Vec<(usize, usize)>> = BTreeMap::new();
        for (t, v) in self.rules.iter().enumerate() {
            for (r, i) in v.iter().enumerate() {
                if let Some(sf) = &i.symface {
                    let face = crate::schema::canon::canonical_json(&sf.face, false)
                        .unwrap_or_else(|_| sf.face.to_string());
                    m.entry((face, sf.element)).or_default().push((t, r));
                }
            }
        }
        m
    }
}

/// Key of [`ImportedSet::symface_groups`]: (the member `face` as canonical JSON, `element.index`).
pub type SymfaceGroup = (String, Option<i64>);

/// The verified group and presented family the `sym` records of a source are checked against.
struct SymCtx {
    family: Family,
    group: VerifiedGroup,
}

/// Fill a hand-name template.
pub fn hand_name(
    t: &HandName,
    source: &str,
    table: Option<&str>,
    r: &RawRecord,
    label: i64,
) -> String {
    let mut origin = r.origin.clone().unwrap_or_default();
    for p in &t.strip {
        if let Some(s) = origin.strip_prefix(p.as_str()) {
            origin = s.to_string();
            break;
        }
    }
    let mut tab = table.unwrap_or("").to_string();
    for p in &t.strip {
        if let Some(s) = tab.strip_prefix(p.as_str()) {
            tab = s.to_string();
            break;
        }
    }
    t.template
        .replace("{source}", source)
        .replace("{label}", &label.to_string())
        .replace("{ordinal}", &r.ordinal.to_string())
        .replace(
            "{index}",
            &r.rec.index.map(|i| i.to_string()).unwrap_or_default(),
        )
        .replace(
            "{origin_index}",
            &r.origin_index.map(|i| i.to_string()).unwrap_or_default(),
        )
        .replace("{origin}", &origin)
        .replace("{table}", &tab)
}

/// One table file of a source to dispatch.
struct FileUnit {
    path: PathBuf,
    table_label: Option<String>,
    expected_sha: Option<String>,
}

/// Load the spec. Relative paths resolve against `base`. (= [`load_with`] with default options: temporary indexes.)
pub fn load(spec: &TableSetSpec, base: &Path) -> Result<ImportedSet, ImportError> {
    load_with(spec, base, &LoadOptions::default())
}

/// The frame source of the spec and the header line of its first table file (the file itself is verified when its
/// source is loaded).
fn frame_header<'a>(
    spec: &'a TableSetSpec,
    base: &Path,
) -> Result<(&'a SourceSpec, TableHeader), ImportError> {
    let fsrc = spec
        .sources
        .iter()
        .find(|s| s.id == spec.frame_from)
        .ok_or_else(|| ImportError::Spec("frame_from".into()))?;
    let funits = file_units(fsrc, base)?;
    let first = funits.first().ok_or_else(|| ImportError::Source {
        source: fsrc.id.clone(),
        msg: "no table file".into(),
    })?;
    let fhead = super::lines::read_header(&fsrc.id, &first.path)?;
    Ok((fsrc, fhead))
}

/// The node frame of the spec (from the header line of the first file of `frame_from`; no table is read): the same
/// frame [`load_with`] gives the loaded set.
pub fn frame_of(spec: &TableSetSpec, base: &Path) -> Result<NodeFrame, ImportError> {
    let (fsrc, fhead) = frame_header(spec, base)?;
    NodeFrame::from_header(&fhead).map_err(|e| ImportError::Source {
        source: fsrc.id.clone(),
        msg: format!("frame: {e}"),
    })
}

/// Load the spec with [`LoadOptions`]. Every table file is read through its index (`import::index`): ONE streaming
/// pass over a table that has no valid index (domains + sha256 digests + rhs keys), none over a table whose index is
/// reused; the bodies are then read from the index.
pub fn load_with(
    spec: &TableSetSpec,
    base: &Path,
    lopts: &LoadOptions,
) -> Result<ImportedSet, ImportError> {
    load_inner(spec, base, lopts, false, false)
}

/// [`load_with`] for callers audited for eps-shifted (two-sided) families (lane epsframe): the frame is read with
/// `NodeFrame::from_header_eps_aware`; on an integer family identical to [`load_with`]. The consumers audited so far:
/// the import's own load, the closure walk ([`crate::import::check`] through `closure`), the fact pass and the
/// evidence lookups (offset + record_sha256). Every other consumer of the set must check `set.frame.has_eps_axes()`.
pub fn load_with_eps(
    spec: &TableSetSpec,
    base: &Path,
    lopts: &LoadOptions,
) -> Result<ImportedSet, ImportError> {
    load_inner(spec, base, lopts, false, true)
}

/// [`load_with`]; `validate_bodies_only` (the classify-only sweep, `import::sweep`): the body pass streams every
/// dispatched record's rhs keys and parses each one as a key pattern of the frame's arity, but KEEPS none (bounded
/// memory on tables of several GB); the returned set has no bodies and must not be checked.
pub(crate) fn load_inner(
    spec: &TableSetSpec,
    base: &Path,
    lopts: &LoadOptions,
    validate_bodies_only: bool,
    eps_aware: bool,
) -> Result<ImportedSet, ImportError> {
    let spec_digest = spec.digest();
    // open table indexes by file (kept for the body pass)
    let mut indexes: BTreeMap<PathBuf, TableIndex> = BTreeMap::new();
    // ---- the frame: the header line of the first file of `frame_from`
    let (fsrc, fhead) = frame_header(spec, base)?;
    // epsframe: an eps-shifted (two-sided) family is accepted by the CLASSIFY-ONLY load and by load_with_eps only
    // (frame, charts, domains, labels (chart, index), dispatcher, bodies); a campaign load refuses it, typed: the
    // partition, loci and termmin read line signs (integer-only; interfaces/campaign-core-domain.md "Eps")
    let frame = if validate_bodies_only || eps_aware {
        NodeFrame::from_header_eps_aware(&fhead)
    } else {
        NodeFrame::from_header(&fhead).map_err(|e| match NodeFrame::from_header_eps_aware(&fhead) {
            Ok(f) => FrameError::Unsupported(format!(
                "eps-shifted family (eps axes {:?}, kshift {:?}): a campaign load (closure, check, partition, \
                 loci, termmin) is integer-only; only the classify-only load and load_with_eps accept it",
                f.eps_axes(),
                f.kshift()
            )),
            Err(_) => e,
        })
    }
    .map_err(|e| ImportError::Source {
        source: fsrc.id.clone(),
        msg: format!("frame: {e}"),
    })?;
    let cmp = match &spec.order {
        Some(o) => NodeComparator::from_header(o),
        None => NodeComparator::from_header(&fhead.raw),
    }
    .map_err(|e| ImportError::Source {
        source: fsrc.id.clone(),
        msg: format!("comparator: {e:?}"),
    })?;
    let arity = frame.arity();
    if cmp.arity() != arity {
        return Err(ImportError::Spec(format!(
            "comparator arity {} differs from the frame arity {arity}",
            cmp.arity()
        )));
    }
    let frame_desc = format!("{:?}", cmp.equivalent_descriptor());
    let codec = KeyCodec::new(arity, spec.bits).map_err(|e| ImportError::Core(format!("{e:?}")))?;

    let mut tables: Vec<Table> = vec![];
    // per dispatcher table: (file, source id, [(ordinal, record sha256)]) for the second (body) pass
    let mut body_plan: Vec<BodyPlan> = vec![];
    let mut rules: Vec<Vec<RuleInfo>> = vec![];
    let mut strict_domains: HashMap<(usize, usize), CaseDomain> = HashMap::new();
    let mut fact_tables = BTreeMap::new();
    let mut table_digests = BTreeMap::new();
    let mut supplied: Vec<Arc<RelationSet>> = vec![];
    let mut supplied_info = BTreeMap::new();
    let mut recipes = vec![];
    let mut terminals_preferred = vec![];
    let mut terminals_other = vec![];
    let mut terminal_provenance: BTreeMap<Vec<i32>, TerminalDecl> = BTreeMap::new();
    let mut credits: Option<CreditPolicy> = None;
    let mut reports = vec![];
    let mut classes = BTreeMap::new();
    let mut scope_labels: BTreeMap<String, BTreeMap<i64, String>> = BTreeMap::new();
    // relation sets wait for the name of the next table to fix their slot
    let mut pending: Vec<Pending> = vec![];
    let mut rel_order: Vec<RelEntry> = vec![];

    for src in &spec.sources {
        let class = src.class_word();
        classes.insert(src.id.clone(), class.clone());
        let mut rep = SourceReport {
            id: src.id.clone(),
            kind: format!("{:?}", src.kind),
            class: class.clone(),
            ..Default::default()
        };
        if frame.has_eps_axes()
            && !matches!(
                src.kind,
                SourceKind::EngineTable | SourceKind::TableSet | SourceKind::Terminals
            )
        {
            // supplied relations, recipes and symmetry credits read the line mask (integer-only)
            return Err(ImportError::Unsupported(format!(
                "source {} ({:?}) on an eps-shifted family: only table and terminal sources are read for eps \
                 families (lane epsframe)",
                src.id, src.kind
            )));
        }
        match src.kind {
            SourceKind::EngineTable | SourceKind::TableSet => {
                let units = file_units(src, base)?;
                let removed: BTreeMap<i64, String> = src
                    .removed
                    .iter()
                    .map(|r| (r.label, r.reason.clone()))
                    .collect();
                let mut seen_removed = BTreeSet::new();
                let mut seen_src_labels: BTreeSet<i64> = BTreeSet::new();
                let mut first_table_name: Option<String> = None;
                let mut sym_ctx: Option<SymCtx> = None;
                let mut sym_err: Option<String> = None;
                for u in &units {
                    let want: Option<Vec<i64>> = src.records.clone();
                    // lane bigline: the index verified against the TABLE (size, mtime, sampled digest, record
                    // count, file digests, spot-checked records); refused and rebuilt once on a failed check
                    let (idx, mut origin, raw, rebuilt) = open_scan_checked(
                        &src.id,
                        &u.path,
                        lopts.index_dir.as_deref(),
                        spec.read_limits(),
                        spec.spot_check_records(),
                    )?;
                    if rebuilt.is_some() {
                        origin.how = "rebuilt_after_failed_check";
                    }
                    rep.indexes.push(origin);
                    indexes.insert(u.path.clone(), idx);
                    let sha = format!("sha256:{}", raw.file_sha256);
                    if let Some(e) = &u.expected_sha {
                        let e = e.trim_start_matches("sha256:");
                        if !e.eq_ignore_ascii_case(&raw.file_sha256) {
                            return Err(ImportError::Digest {
                                path: u.path.display().to_string(),
                                expected: e.to_string(),
                                got: raw.file_sha256.clone(),
                            });
                        }
                    }
                    check_header(&src.id, &raw.header, &frame, &frame_desc, &fhead)?;
                    // lane import5 (interfaces/import.md 18.2 / 18.3): the table's own statements about itself
                    let policy = MarkerPolicy {
                        accept_fewer_rules: src
                            .marker_rules
                            .as_ref()
                            .map(|m| m.accept_fewer.clone()),
                    };
                    let ms = check_markers(&src.id, &u.path, &raw, arity, &policy)?;
                    rep.marker_overrides.extend(ms.overrides.iter().cloned());
                    for k in [
                        "assembly_provenance",
                        "composite_assembly_complete",
                        "assembly_end",
                        "composite_terminal_ledger",
                    ] {
                        if let Some(n) = raw.kinds.get(k) {
                            *rep.markers.entry(k.to_string()).or_default() += n;
                        }
                    }
                    if ms.assembly {
                        rep.assembly_files.push(u.path.display().to_string());
                    }
                    let tt = src.table_terminals.unwrap_or_default();
                    let mut declared: Vec<(&'static str, usize, Vec<i32>)> = vec![];
                    if tt != TableTerminals::None {
                        declared.extend(
                            ms.ledger_keys
                                .iter()
                                .map(|(l, k)| ("composite_terminal_ledger", *l, k.clone())),
                        );
                    }
                    if tt == TableTerminals::LedgerAndResiduals {
                        if ms.residual_keys.len() != raw.markers.residuals.len() {
                            return Err(ImportError::Source {
                                source: src.id.clone(),
                                msg: format!(
                                    "{}: table_terminals ledger_and_residuals: {} of {} residual keys are not integer keys of the arity",
                                    u.path.display(),
                                    raw.markers.residuals.len() - ms.residual_keys.len(),
                                    raw.markers.residuals.len()
                                ),
                            });
                        }
                        declared.extend(
                            ms.residual_keys
                                .iter()
                                .map(|(l, k)| ("residual", *l, k.clone())),
                        );
                    }
                    for (prov, line, k) in declared {
                        if terminals_preferred.contains(&k)
                            || terminals_other.contains(&k)
                            || terminal_provenance.contains_key(&k)
                        {
                            continue;
                        }
                        terminal_provenance.insert(
                            k.clone(),
                            TerminalDecl {
                                provenance: prov,
                                source: src.id.clone(),
                                path: u.path.display().to_string(),
                                line,
                            },
                        );
                        terminals_other.push(k);
                        rep.table_terminals += 1;
                    }
                    // `sym` records: the verified group of the source, once (interfaces/import.md 18.4)
                    if raw.sym_lines > 0
                        && sym_ctx.is_none()
                        && sym_err.is_none()
                        && frame.has_eps_axes()
                    {
                        // the seven-step check reads the line mask (integer-only): the sym records stay unverified
                        sym_err = Some(
                            "eps-shifted family: sym-record verification is integer-only (lane epsframe)".into(),
                        );
                    }
                    if raw.sym_lines > 0 && sym_ctx.is_none() && sym_err.is_none() {
                        let gs = src.sym_group.clone().unwrap_or(GroupSpec {
                            family: None,
                            group: None,
                            group_format: GroupFormat::Derive,
                            max_candidates: None,
                            max_terms: None,
                        });
                        match load_verified_group_family(
                            &gs,
                            base,
                            frame.mask(),
                            cmp.presentation().new_to_old(),
                            fhead.raw.get("family_source").and_then(Value::as_str),
                        ) {
                            Ok((group, _, family)) => sym_ctx = Some(SymCtx { family, group }),
                            Err(e) => sym_err = Some(e.to_string()),
                        }
                    }
                    rep.files.push((u.path.display().to_string(), sha.clone()));
                    rep.records_in_files += raw.records.len();
                    for (k, n) in &raw.kinds {
                        *rep.line_kinds.entry(k.clone()).or_default() += n;
                    }
                    if !raw.kinds.contains_key("end") {
                        rep.files_without_end.push(u.path.display().to_string());
                    }
                    // labels unique inside the file / source / scope (eps family: the label is (chart, index), packed
                    // as the cover table packs it: the engine restarts `index` in every chart)
                    let mut by_label: BTreeMap<i64, usize> = BTreeMap::new();
                    for (i, r) in raw.records.iter().enumerate() {
                        let l = record_label(&frame, &r.rec).ok_or_else(|| ImportError::Line {
                            source: src.id.clone(),
                            path: u.path.display().to_string(),
                            line: r.line_no,
                            msg: if frame.has_eps_axes() {
                                "rule record without index, or its chart is not a chart of the eps frame".into()
                            } else {
                                "rule record without index".into()
                            },
                        })?;
                        if by_label.insert(l, i).is_some() || !seen_src_labels.insert(l) {
                            return Err(ImportError::Label {
                                source: src.id.clone(),
                                label: l,
                                msg: format!("repeated in {}", u.path.display()),
                            });
                        }
                        if let Some(scope) = &src.label_scope
                            && scope != "source"
                        {
                            let m = scope_labels.entry(scope.clone()).or_default();
                            if let Some(prev) = m.insert(l, src.id.clone()) {
                                return Err(ImportError::Label {
                                    source: src.id.clone(),
                                    label: l,
                                    msg: format!(
                                        "repeated in label scope {scope} (also in source {prev})"
                                    ),
                                });
                            }
                        }
                    }
                    // selection, in the given order
                    let mut order: Vec<usize> = match &want {
                        None => (0..raw.records.len()).collect(),
                        Some(sel) => {
                            let mut v = vec![];
                            for l in sel {
                                if let Some(&i) = by_label.get(l) {
                                    v.push(i);
                                } else if units.len() == 1 {
                                    return Err(ImportError::Label {
                                        source: src.id.clone(),
                                        label: *l,
                                        msg: "selected but not in the file".into(),
                                    });
                                }
                            }
                            v
                        }
                    };
                    order.retain(|&i| {
                        let l = record_label(&frame, &raw.records[i].rec).unwrap_or(i64::MIN);
                        if let Some(reason) = removed.get(&l) {
                            seen_removed.insert(l);
                            rep.removed.push((l, reason.clone()));
                            false
                        } else {
                            true
                        }
                    });
                    // one strict table per file (witness check ON for every record; elided / witness-absent records
                    // serve only through the closure's per-key lift, closure.md section 16)
                    let name = match &u.table_label {
                        Some(t) => format!("{}/{}", src.id, t),
                        None => src.id.clone(),
                    };
                    if !order.is_empty() {
                        let witness = spec.policies.witness_check == super::spec::WitnessCheck::On;
                        let sym = match (&sym_ctx, &sym_err) {
                            (Some(c), _) => Ok(c),
                            (None, Some(e)) => Err(e.as_str()),
                            (None, None) => Err("no verified group"),
                        };
                        let (t, tf, infos) = build_table(
                            src, &class, &name, u, &raw, &order, &frame, witness, sym, &mut rep,
                        )?;
                        if !witness {
                            // the strict domains of the records with complete witness loci (diagnostic runs)
                            for (k, info) in infos.iter().enumerate() {
                                if info.witness != WitnessKind::Complete {
                                    continue;
                                }
                                let rec = &raw.records[order[t.meta[k].source_index]].rec;
                                let mut d = CaseDomain::from_rule_record(
                                    rec,
                                    &frame,
                                    DomainOptions {
                                        witness: true,
                                        ..Default::default()
                                    },
                                )
                                .map_err(|e| {
                                    ImportError::Source {
                                        source: src.id.clone(),
                                        msg: format!("strict domain of label {}: {e}", info.label),
                                    }
                                })?;
                                d.bounds = t.domains[k].bounds.clone();
                                strict_domains.insert((tables.len(), k), d);
                            }
                        }
                        if !t.is_empty() {
                            if first_table_name.is_none() {
                                first_table_name = Some(name.clone());
                            }
                            fact_tables.insert(name.clone(), u.path.display().to_string());
                            table_digests.insert(name.clone(), sha.clone());
                            rep.tables.push(name.clone());
                            rep.records_dispatched += t.len();
                            body_plan.push((
                                u.path.clone(),
                                src.id.clone(),
                                infos
                                    .iter()
                                    .map(|i| (i.ordinal, i.record_sha256.clone()))
                                    .collect(),
                            ));
                            drop(tf);
                            tables.push(t);
                            rules.push(infos);
                        }
                    }
                }
                if let Some(sel) = &src.records
                    && units.len() > 1
                {
                    for l in sel {
                        if !seen_src_labels.contains(l) {
                            return Err(ImportError::Label {
                                source: src.id.clone(),
                                label: *l,
                                msg: "selected but in no table of the set".into(),
                            });
                        }
                    }
                }
                for l in removed.keys() {
                    if !seen_removed.contains(l) {
                        return Err(ImportError::Label {
                            source: src.id.clone(),
                            label: *l,
                            msg: "removed but not present (or not selected)".into(),
                        });
                    }
                }
                if let Some(n) = first_table_name {
                    flush_pending(
                        &mut pending,
                        DispatchSlot::BeforeTable(n),
                        &mut supplied,
                        &mut recipes,
                        &mut rel_order,
                    );
                }
            }
            SourceKind::SuppliedRelations => {
                let (rs, info) = load_supplied(
                    src,
                    &class,
                    base,
                    &frame,
                    &frame_desc,
                    &fhead,
                    &cmp,
                    codec,
                    &mut rep,
                    spec.read_limits(),
                )?;
                supplied_info.insert(rs.name.clone(), info);
                pending.push(Pending::Supplied(rs));
            }
            SourceKind::Recipe => {
                let rc = load_recipe(src, &class, base, &frame)?;
                rep.files
                    .push((rc.path.clone(), format!("sha256:{}", rc.recipe_sha256)));
                pending.push(Pending::Recipe(rc));
            }
            SourceKind::Terminals => {
                let keys = match (&src.keys, &src.path) {
                    (Some(k), None) => k.clone(),
                    (None, Some(p)) => {
                        let path = resolve(base, p);
                        // lane closeloop4 (audit-closeloop M1 / m7): a terminals FILE is pinned (it decides keys)
                        let Some(want) = src.sha256.as_deref() else {
                            return Err(ImportError::Spec(format!(
                                "source {}: the terminals file {} is not pinned: give its sha256",
                                src.id,
                                path.display()
                            )));
                        };
                        let d = verify_file(&path, Some(want))?;
                        rep.files
                            .push((path.display().to_string(), format!("sha256:{d}")));
                        read_key_list(&path, arity)?
                    }
                    _ => return Err(ImportError::Spec(format!("source {}: keys / path", src.id))),
                };
                for k in &keys {
                    if k.len() != arity {
                        return Err(ImportError::Source {
                            source: src.id.clone(),
                            msg: format!("terminal key of length {} for arity {arity}", k.len()),
                        });
                    }
                }
                // a key a table already declared stays declared once (with its table provenance); a preferred
                // declaration moves it to the preferred list (lane import5)
                for k in keys {
                    if src.preferred.unwrap_or(false) {
                        if let Some(pos) = terminals_other.iter().position(|x| *x == k)
                            && terminal_provenance.remove(&k).is_some()
                        {
                            terminals_other.remove(pos);
                        }
                        terminals_preferred.push(k);
                    } else if !terminal_provenance.contains_key(&k) {
                        terminals_other.push(k);
                    }
                }
            }
            SourceKind::SymmetryCredit => {
                if credits.is_some() {
                    return Err(ImportError::Spec(
                        "more than one symmetry_credit source".into(),
                    ));
                }
                if src.family.is_some() || src.group_format.is_some() {
                    // the VERIFIED group: derived from the family (presented into the tables' frame), supplied data
                    // checked against it (interfaces/import.md 13.1)
                    let gs = GroupSpec {
                        family: src.family.clone(),
                        group: src.group.clone(),
                        group_format: src.group_format.unwrap_or(if src.group.is_some() {
                            GroupFormat::Symgroup
                        } else {
                            GroupFormat::Derive
                        }),
                        max_candidates: src.max_candidates,
                        max_terms: None,
                    };
                    let (vg, gr) = load_verified_group(
                        &gs,
                        base,
                        frame.mask(),
                        cmp.presentation().new_to_old(),
                        fhead.raw.get("family_source").and_then(Value::as_str),
                    )?;
                    rep.files
                        .push((gr.family.clone(), format!("sha256:{}", gr.family_sha256)));
                    if let Some(f) = &gr.group_file {
                        rep.files.push(f.clone());
                    }
                    let pol = CreditPolicy::from_verified(&vg, frame.mask()).map_err(|m| {
                        ImportError::Source {
                            source: src.id.clone(),
                            msg: format!("verified group -> credit policy: {m}"),
                        }
                    })?;
                    rep.group = Some(gr);
                    credits = Some(pol);
                } else {
                    let g = src
                        .group
                        .as_ref()
                        .ok_or_else(|| ImportError::Spec("group".into()))?;
                    let path = resolve(base, &g.path);
                    let d = verify_file(&path, g.sha256.as_deref())?;
                    rep.files
                        .push((path.display().to_string(), format!("sha256:{d}")));
                    credits = Some(read_credit_group(&path, frame.mask())?);
                }
            }
        }
        reports.push(rep);
    }
    // relation sets after the last table
    flush_pending(
        &mut pending,
        DispatchSlot::Last,
        &mut supplied,
        &mut recipes,
        &mut rel_order,
    );
    if tables.is_empty() {
        return Err(ImportError::Spec("no dispatcher table in the spec".into()));
    }
    let n = tables.len();
    let disp = Dispatcher::new(tables, (0..n).collect())
        .map_err(|e| ImportError::Core(format!("dispatcher: {e}")))?;
    // second pass, one FILE at a time (bounded memory): the rhs keys of the dispatched records, each line's sha256
    // re-checked against the first pass (a file that changed between the passes is refused)
    let mut bodies = RuleBodies::new(&disp);
    let mut by_file: BTreeMap<PathBuf, Vec<usize>> = BTreeMap::new();
    for (t, (p, _, _)) in body_plan.iter().enumerate() {
        by_file.entry(p.clone()).or_default().push(t);
    }
    for (path, ts) in &by_file {
        // ordinal -> (expected sha256, [(table, record position)])
        type Places<'a> = (&'a str, Vec<(usize, usize)>);
        let mut want: HashMap<usize, Places> = HashMap::new();
        for &t in ts {
            for (k, (o, sha)) in body_plan[t].2.iter().enumerate() {
                want.entry(*o)
                    .or_insert((sha.as_str(), vec![]))
                    .1
                    .push((t, k));
            }
        }
        let src_id = &body_plan[ts[0]].1;
        let idx = indexes
            .get(path)
            .ok_or_else(|| ImportError::Core(format!("no index for {}", path.display())))?;
        idx.scan_bodies(&|o| want.contains_key(&o), &mut |o, rsha, rhs| {
            let (sha, places) = &want[&o];
            if *sha != rsha {
                return Err(ImportError::Source {
                    source: src_id.clone(),
                    msg: format!(
                        "index of {}: body of record {o} has sha256 {rsha}, its domain entry {sha}",
                        path.display()
                    ),
                });
            }
            if validate_bodies_only {
                for k in &rhs {
                    // the parse of `RuleBodies::set` and its i8 ranges (offsets, constants, parameters)
                    use crate::closure::bodies::{RhsSlot, parse_rhs_pattern};
                    let ok = parse_rhs_pattern(k, arity).is_ok_and(|p| {
                        p.iter().all(|a| match *a {
                            RhsSlot::Param { param, offset } => {
                                i8::try_from(param).is_ok() && i8::try_from(offset).is_ok()
                            }
                            RhsSlot::Const(c) => i8::try_from(c).is_ok(),
                        })
                    });
                    if !ok {
                        return Err(ImportError::Source {
                            source: src_id.clone(),
                            msg: format!(
                                "{}: record {o}: rhs key {k:?} is not a key pattern of arity {arity}",
                                path.display()
                            ),
                        });
                    }
                }
                return Ok(());
            }
            for &(t, k) in places {
                bodies.set(&disp, t, k, &rhs).map_err(|e| {
                    ImportError::Core(format!("bodies of {}: {e:?}", disp.tables()[t].name))
                })?;
            }
            Ok(())
        })?;
    }
    drop(indexes);
    for t in 0..disp.tables().len() {
        if validate_bodies_only {
            break;
        }
        let with = bodies.count_with_body(t);
        if with != disp.tables()[t].len() {
            return Err(ImportError::Source {
                source: disp.tables()[t].name.clone(),
                msg: format!("{} of {} records have a body", with, disp.tables()[t].len()),
            });
        }
    }
    drop(body_plan);
    let mut by_sha = HashMap::new();
    for (t, v) in rules.iter().enumerate() {
        for (r, info) in v.iter().enumerate() {
            if let Some(prev) = by_sha.insert(info.record_sha256.clone(), (t, r)) {
                let p = &rules[prev.0][prev.1];
                return Err(ImportError::Source {
                    source: info.source.clone(),
                    msg: format!(
                        "record {} (label {}) is byte-identical to {} label {} (sha256 {}): one record cannot be dispatched twice",
                        info.table_name, info.label, p.table_name, p.label, info.record_sha256
                    ),
                });
            }
        }
    }
    if frame.has_eps_axes() && spec.start.format == StartFormat::Box {
        // the box enumerates line powers >= 1 only (boxdef is integer-only; the eps box, BOXDEF B, is spec data not
        // implemented): a box start on an eps family would silently miss every key with an eps axis <= 0
        return Err(ImportError::Unsupported(
            "start format box on an eps-shifted family (boxdef is integer-only; give the start keys)".into(),
        ));
    }
    let start = read_start(spec, base, codec)?;
    let records = rules.iter().map(Vec::len).sum();
    let report = LoadReport {
        spec_digest,
        node: spec.node.clone(),
        arity,
        mask: frame.mask_string(),
        comparator: cmp.name().to_string(),
        start_keys: start.len(),
        start_digest: start.digest(),
        sources: reports,
        tables: disp.tables().len(),
        records,
        relation_sets: supplied.len() + recipes.len(),
        terminals_preferred: terminals_preferred.len(),
        terminals_other: terminals_other.len(),
        terminals_from_tables: terminal_provenance.len(),
    };
    Ok(ImportedSet {
        spec: spec.clone(),
        frame,
        cmp,
        codec,
        disp,
        bodies,
        rules,
        fact_tables,
        table_digests,
        supplied,
        supplied_info,
        recipes,
        rel_order,
        terminals_preferred,
        terminals_other,
        credits,
        start,
        by_sha,
        classes,
        report,
        strict_domains,
        family: fhead.family.clone(),
        terminal_provenance,
    })
}

/// Per dispatcher table: (file, source id, [(ordinal, record sha256)]) for the body pass.
type BodyPlan = (PathBuf, String, Vec<(usize, String)>);

enum Pending {
    Supplied(RelationSet),
    Recipe(RecipeSource),
}

fn flush_pending(
    pending: &mut Vec<Pending>,
    slot: DispatchSlot,
    supplied: &mut Vec<Arc<RelationSet>>,
    recipes: &mut Vec<RecipeSource>,
    order: &mut Vec<RelEntry>,
) {
    for p in pending.drain(..) {
        match p {
            Pending::Supplied(mut rs) => {
                rs.slot = slot.clone();
                order.push(RelEntry::Supplied(supplied.len()));
                supplied.push(Arc::new(rs));
            }
            Pending::Recipe(mut rc) => {
                rc.slot = slot.clone();
                order.push(RelEntry::Recipe(recipes.len()));
                recipes.push(rc);
            }
        }
    }
}

fn file_units(src: &SourceSpec, base: &Path) -> Result<Vec<FileUnit>, ImportError> {
    match src.kind {
        SourceKind::EngineTable | SourceKind::SuppliedRelations => Ok(vec![FileUnit {
            path: resolve(base, src.path.as_deref().unwrap_or_default()),
            table_label: None,
            expected_sha: src.sha256.clone(),
        }]),
        SourceKind::TableSet => {
            let pf = resolve(base, src.priority_file.as_deref().unwrap_or_default());
            let txt = std::fs::read_to_string(&pf).map_err(|e| io_err(&pf, e))?;
            let v: Value = serde_json::from_str(&txt).map_err(|e| ImportError::Source {
                source: src.id.clone(),
                msg: format!("{}: {e}", pf.display()),
            })?;
            let variant = src.variant.as_deref().unwrap_or_default();
            let list =
                v.get(variant)
                    .and_then(Value::as_array)
                    .ok_or_else(|| ImportError::Source {
                        source: src.id.clone(),
                        msg: format!("{}: no variant {variant:?}", pf.display()),
                    })?;
            let tdir = resolve(base, src.tables_dir.as_deref().unwrap_or_default());
            let mut sums: BTreeMap<String, String> = BTreeMap::new();
            for s in &src.sha256sums {
                let p = resolve(base, &s.path);
                verify_file(&p, s.sha256.as_deref())?;
                for (k, h) in read_sha256sums(&p)? {
                    if let Some(old) = sums.insert(k.clone(), h.clone())
                        && old != h
                    {
                        return Err(ImportError::Source {
                            source: src.id.clone(),
                            msg: format!("{k} has two digests in the SHA256SUMS files"),
                        });
                    }
                }
            }
            let mut out = vec![];
            let mut last_prio: Option<i64> = None;
            let mut origins_seen = BTreeSet::new();
            for e in list {
                let file =
                    e.get("file")
                        .and_then(Value::as_str)
                        .ok_or_else(|| ImportError::Source {
                            source: src.id.clone(),
                            msg: format!("priority entry without file: {e}"),
                        })?;
                let origin = e
                    .get("origin")
                    .and_then(Value::as_str)
                    .unwrap_or(file)
                    .to_string();
                if !origins_seen.insert(origin.clone()) {
                    return Err(ImportError::Source {
                        source: src.id.clone(),
                        msg: format!("origin {origin} listed twice"),
                    });
                }
                if let Some(p) = e.get("priority").and_then(Value::as_i64) {
                    if last_prio.is_some_and(|l| p <= l) {
                        return Err(ImportError::Source {
                            source: src.id.clone(),
                            msg: format!("priority list not ascending at {origin}"),
                        });
                    }
                    last_prio = Some(p);
                }
                if src
                    .include_origins
                    .as_ref()
                    .is_some_and(|v| !v.contains(&origin))
                    || src
                        .exclude_origins
                        .as_ref()
                        .is_some_and(|v| v.contains(&origin))
                {
                    continue;
                }
                let entry_sha = e.get("sha256").and_then(Value::as_str).map(str::to_string);
                let sums_sha = sums.get(file).cloned();
                if !src.sha256sums.is_empty() && sums_sha.is_none() {
                    return Err(ImportError::Source {
                        source: src.id.clone(),
                        msg: format!("{file} is not listed in the SHA256SUMS files"),
                    });
                }
                if let (Some(a), Some(b)) = (&entry_sha, &sums_sha)
                    && !a.eq_ignore_ascii_case(b)
                {
                    return Err(ImportError::Source {
                        source: src.id.clone(),
                        msg: format!(
                            "{file}: priority file sha256 {a} differs from SHA256SUMS {b}"
                        ),
                    });
                }
                out.push(FileUnit {
                    path: tdir.join(file),
                    table_label: Some(origin),
                    expected_sha: sums_sha.or(entry_sha),
                });
            }
            if let Some(v) = &src.include_origins {
                for o in v {
                    if !origins_seen.contains(o) {
                        return Err(ImportError::Source {
                            source: src.id.clone(),
                            msg: format!("include_origins names {o}, not in the priority file"),
                        });
                    }
                }
            }
            Ok(out)
        }
        _ => Ok(vec![]),
    }
}

/// The label of a rule record in the import: `i`, else `index`; on an eps-shifted family the PACKED (chart, index)
/// of the cover table ([`crate::cover::eps_label`]: the engine restarts `index` in every chart). `None` without an
/// index, or (eps) with a chart that is not a chart of the frame.
fn record_label(frame: &NodeFrame, rec: &crate::record::RuleRecord) -> Option<i64> {
    let l = rec.i.or(rec.index)?;
    if !frame.has_eps_axes() {
        return Some(l);
    }
    let bits: Vec<bool> = rec.chart.as_deref()?.chars().map(|x| x == '1').collect();
    let ord = frame.charts().iter().position(|c| *c == bits)?;
    crate::cover::eps_label(ord, l)
}

fn check_header(
    source: &str,
    h: &TableHeader,
    frame: &NodeFrame,
    frame_desc: &str,
    fhead: &TableHeader,
) -> Result<(), ImportError> {
    let f = if frame.has_eps_axes() {
        NodeFrame::from_header_eps_aware(h)
    } else {
        NodeFrame::from_header(h)
    }
    .map_err(|e| ImportError::Source {
        source: source.into(),
        msg: format!("frame: {e}"),
    })?;
    if f.arity() != frame.arity()
        || !f.same_mask(frame)
        || f.power_shifts() != frame.power_shifts()
        || f.kshift() != frame.kshift()
    {
        return Err(ImportError::Source {
            source: source.into(),
            msg: format!(
                "frame (arity {}, mask {}) differs from the set's (arity {}, mask {})",
                f.arity(),
                f.mask_string(),
                frame.arity(),
                frame.mask_string()
            ),
        });
    }
    if let (Some(a), Some(b)) = (&h.family, &fhead.family)
        && a != b
    {
        return Err(ImportError::Source {
            source: source.into(),
            msg: format!("family {a} differs from {b}"),
        });
    }
    // the order, where the header states one
    if h.raw.get("node_order").is_some() || h.raw.get("order").is_some() {
        let c = NodeComparator::from_header(&h.raw).map_err(|e| ImportError::Source {
            source: source.into(),
            msg: format!("comparator: {e:?}"),
        })?;
        let d = format!("{:?}", c.equivalent_descriptor());
        if d != frame_desc {
            return Err(ImportError::Source {
                source: source.into(),
                msg: "the header's order differs from the set's comparator".into(),
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_table(
    src: &SourceSpec,
    class: &str,
    name: &str,
    u: &FileUnit,
    raw: &RawTable,
    seg: &[usize],
    frame: &NodeFrame,
    witness_check: bool,
    sym: Result<&SymCtx, &str>,
    rep: &mut SourceReport,
) -> Result<(Table, TableFile, Vec<RuleInfo>), ImportError> {
    let tf = TableFile {
        header: Some(raw.header.clone()),
        rules: seg.iter().map(|&i| raw.records[i].rec.clone()).collect(),
        charts: raw.charts.clone(),
        others: vec![],
    };
    let opts = DomainOptions {
        witness: witness_check,
        ..Default::default()
    };
    let loaded = if frame.has_eps_axes() {
        Table::from_table_file_eps_aware(name, u.path.display().to_string(), &tf, opts)
    } else {
        Table::from_table_file(name, u.path.display().to_string(), &tf, opts)
    };
    let mut t = loaded.map_err(|e| ImportError::Source {
        source: src.id.clone(),
        msg: format!("{}: {e}", u.path.display()),
    })?;
    for c in &t.provenance.chart_guards_implied {
        rep.chart_guards_implied
            .push((u.path.display().to_string(), c.clone()));
    }
    for s in &t.provenance.skipped_not_dispatched {
        rep.not_dispatched
            .push((name.to_string(), s.label, s.dispatch.clone()));
    }
    let mut infos = vec![];
    for k in 0..t.len() {
        let r = &raw.records[seg[t.meta[k].source_index]];
        let label = t.labels[k];
        let mut bounds: Vec<Poly> = vec![];
        if src.domain == DomainMode::QualifiedRecenter && frame.has_eps_axes() {
            return Err(ImportError::Unsupported(format!(
                "source {}: domain qualified_recenter on an eps-shifted family (a floor is not defined on an eps axis)",
                src.id
            )));
        }
        if src.domain == DomainMode::QualifiedRecenter {
            let rc = r.rec.recenter.as_ref().ok_or_else(|| ImportError::Line {
                source: src.id.clone(),
                path: u.path.display().to_string(),
                line: r.line_no,
                msg: "domain qualified_recenter needs the record's recenter".into(),
            })?;
            if rc.len() != frame.arity() {
                return Err(ImportError::Line {
                    source: src.id.clone(),
                    path: u.path.display().to_string(),
                    line: r.line_no,
                    msg: format!("recenter of length {}", rc.len()),
                });
            }
            if t.domains[k].target != TargetMap::Canonical {
                return Err(ImportError::Line {
                    source: src.id.clone(),
                    path: u.path.display().to_string(),
                    line: r.line_no,
                    msg: "recenter bounds with a non-canonical target".into(),
                });
            }
            for (i, &c) in rc.iter().enumerate() {
                let c = c as i128;
                if frame.is_line(i) {
                    let lo = 1i128.max(1 - c);
                    if lo > 1 {
                        bounds.push(Poly::slot_minus(i, lo));
                    }
                } else {
                    let hi = 0i128.min(-c);
                    if hi < 0 {
                        bounds.push(
                            Poly::slot_minus(i, hi)
                                .neg()
                                .map_err(|e| ImportError::Core(e.to_string()))?,
                        );
                    }
                }
            }
        }
        if let Some(db) = &r.domain_bounds {
            for i in 0..frame.arity() {
                // engfix4 mn-2: the engine's limit on a FREE slot (one exception branch per excluded value there)
                let free = r.rec.fixed.get(i).is_some_and(Option::is_none);
                let far = db.min[i].is_some_and(|lo| lo - 1 > MAX_BOUND_EXCLUSIONS_PER_SLOT)
                    || db.max[i].is_some_and(|hi| -hi > MAX_BOUND_EXCLUSIONS_PER_SLOT);
                if free && far {
                    return Err(ImportError::Line {
                        source: src.id.clone(),
                        path: u.path.display().to_string(),
                        line: r.line_no,
                        msg: format!(
                            "domain_bounds on the free slot {i} excludes more than {MAX_BOUND_EXCLUSIONS_PER_SLOT} values (refused as by the engine)"
                        ),
                    });
                }
                if let Some(lo) = db.min[i] {
                    if frame.is_eps_axis(i) {
                        return Err(ImportError::Line {
                            source: src.id.clone(),
                            path: u.path.display().to_string(),
                            line: r.line_no,
                            msg: format!(
                                "domain_bounds.min on the eps axis {i} (a floor is not defined on a two-sided axis)"
                            ),
                        });
                    }
                    if !frame.is_line(i) {
                        return Err(ImportError::Line {
                            source: src.id.clone(),
                            path: u.path.display().to_string(),
                            line: r.line_no,
                            msg: format!("domain_bounds.min on the numerator slot {i}"),
                        });
                    }
                    bounds.push(Poly::slot_minus(i, lo as i128));
                }
                if let Some(hi) = db.max[i] {
                    if frame.is_line(i) {
                        return Err(ImportError::Line {
                            source: src.id.clone(),
                            path: u.path.display().to_string(),
                            line: r.line_no,
                            msg: format!("domain_bounds.max on the line slot {i}"),
                        });
                    }
                    bounds.push(
                        Poly::slot_minus(i, hi as i128)
                            .neg()
                            .map_err(|e| ImportError::Core(e.to_string()))?,
                    );
                }
            }
        }
        if !bounds.is_empty() {
            rep.bounds_records += 1;
            t.domains[k].bounds.extend(bounds);
            t.domains[k].validate().map_err(|e| ImportError::Line {
                source: src.id.clone(),
                path: u.path.display().to_string(),
                line: r.line_no,
                msg: format!("domain with bounds: {e}"),
            })?;
        }
        let witness = match t.meta[k].witness {
            WitnessStatus::Complete => WitnessKind::Complete,
            WitnessStatus::Elided => WitnessKind::Elided,
            WitnessStatus::NotCarried => WitnessKind::NotCarried,
        };
        *rep.witness.entry(format!("{witness:?}")).or_default() += 1;
        // a `sym` record: verified by the seven steps against our derived group, else blocked at every key of its
        // case by an always-vanishing pole locus (never serves, never falls through; interfaces/import.md 18.4)
        let sym_info = r.sym.as_ref().map(|s| {
            let v = match sym {
                Ok(c) => verify_sym_record(s, &c.family, &c.group, frame.mask()),
                Err(e) => Err(format!("no verified group: {e}")),
            };
            let srep = rep.sym.get_or_insert_with(SymReport::default);
            srep.records += 1;
            if let Err(e) = &sym {
                srep.group_error = Some(e.to_string());
            }
            match v {
                Ok(el) => {
                    srep.verified += 1;
                    SymInfo {
                        verified: true,
                        reason: None,
                        element: Some(el),
                        action: s.action.clone(),
                    }
                }
                Err(why) => {
                    srep.unverified.push((label, why.clone()));
                    SymInfo {
                        verified: false,
                        reason: Some(why),
                        element: None,
                        action: s.action.clone(),
                    }
                }
            }
        });
        if sym_info.as_ref().is_some_and(|s| !s.verified) {
            t.domains[k].poles.push(PoleLocus {
                kind: PoleKind::Witness,
                coeffs: vec![],
            });
        }
        infos.push(RuleInfo {
            source: src.id.clone(),
            class: class.to_string(),
            table_name: name.to_string(),
            path: u.path.display().to_string(),
            ordinal: r.ordinal,
            label,
            index: r.rec.index,
            offset: r.offset,
            bytes: r.bytes,
            record_sha256: r.sha256.clone(),
            hand_name: src
                .hand_name
                .as_ref()
                .map(|h| hand_name(h, &src.id, u.table_label.as_deref(), r, label)),
            witness,
            sym: sym_info,
            symface: r.symface.clone(),
        });
    }
    Ok((t, tf, infos))
}

#[allow(clippy::too_many_arguments)]
fn load_supplied(
    src: &SourceSpec,
    class: &str,
    base: &Path,
    frame: &NodeFrame,
    frame_desc: &str,
    fhead: &TableHeader,
    cmp: &NodeComparator,
    codec: KeyCodec,
    rep: &mut SourceReport,
    limits: crate::record::ReadLimits,
) -> Result<(RelationSet, BTreeMap<Vec<i32>, SuppliedInfo>), ImportError> {
    let path = resolve(base, src.path.as_deref().unwrap_or_default());
    let raw = scan_table_with(&src.id, &path, true, &|_, _| true, limits)?;
    if let Some(r) = raw.records.iter().find(|r| r.sym.is_some()) {
        return Err(ImportError::Line {
            source: src.id.clone(),
            path: path.display().to_string(),
            line: r.line_no,
            msg: "a sym record in a supplied relation file (only point rule records)".into(),
        });
    }
    if let Some(e) = &src.sha256
        && !e
            .trim_start_matches("sha256:")
            .eq_ignore_ascii_case(&raw.file_sha256)
    {
        return Err(ImportError::Digest {
            path: path.display().to_string(),
            expected: e.clone(),
            got: raw.file_sha256.clone(),
        });
    }
    // a supplied file may carry a hand-written header without the frame fields: then every record's chart must be the
    // set's mask (checked below) and the family, where stated, must agree
    if raw.header.mask.is_some() || raw.header.denominators.is_some() {
        check_header(&src.id, &raw.header, frame, frame_desc, fhead)?;
    } else if let (Some(a), Some(b)) = (&raw.header.family, &fhead.family)
        && a != b
    {
        return Err(ImportError::Source {
            source: src.id.clone(),
            msg: format!("family {a} differs from {b}"),
        });
    }
    rep.files.push((
        path.display().to_string(),
        format!("sha256:{}", raw.file_sha256),
    ));
    let ver = match &src.verification {
        Some(v) => {
            let p = resolve(base, &v.path);
            let d = verify_file(&p, v.sha256.as_deref())?;
            rep.files
                .push((p.display().to_string(), format!("sha256:{d}")));
            Some(d)
        }
        None => None,
    };
    let n = frame.arity();
    // `verify`: the verified group and the coefficient texts of every record (small files: read whole)
    let vgroup = match &src.verify {
        Some(gs) => {
            let (vg, gr) = load_verified_group(
                gs,
                base,
                frame.mask(),
                cmp.presentation().new_to_old(),
                fhead.raw.get("family_source").and_then(Value::as_str),
            )?;
            rep.files
                .push((gr.family.clone(), format!("sha256:{}", gr.family_sha256)));
            rep.group = Some(gr);
            Some((vg, gs.max_terms.unwrap_or(DEFAULT_MAX_TERMS)))
        }
        None => None,
    };
    let coefs: SuppliedCoefficients = if vgroup.is_some() {
        supplied_coefficients(&src.id, &path, n, limits)?
    } else {
        HashMap::new()
    };
    let mut b = RelationStoreBuilder::new(codec);
    let mut info = BTreeMap::new();
    rep.records_in_files = raw.records.len();
    for r in &raw.records {
        let lerr = |m: String| ImportError::Line {
            source: src.id.clone(),
            path: path.display().to_string(),
            line: r.line_no,
            msg: m,
        };
        if r.rec.chart.as_deref() != Some(frame.mask_string().as_str()) {
            return Err(lerr(format!(
                "a supplied relation must state the chart {} (has {:?})",
                frame.mask_string(),
                r.rec.chart
            )));
        }
        let key: Vec<i32> = r
            .rec
            .fixed
            .iter()
            .map(|x| x.and_then(|v| i32::try_from(v).ok()))
            .collect::<Option<Vec<i32>>>()
            .filter(|k| k.len() == n)
            .ok_or_else(|| {
                lerr("a supplied relation must be a point record (every slot fixed)".into())
            })?;
        if let Some(t) = &r.rec.target
            && parse_key_text(t, n).as_deref() != Some(&key[..])
        {
            return Err(lerr(format!("target {t} differs from the fixed slots")));
        }
        let rhs_txt = r
            .rec
            .rhs_keys
            .as_ref()
            .ok_or_else(|| lerr("no rhs".into()))?;
        let mut rhs = vec![];
        let mut descends = true;
        for t in rhs_txt {
            let k = parse_key_text(t, n)
                .ok_or_else(|| lerr(format!("rhs key {t} is not a constant key")))?;
            let top = (0..n).all(|i| {
                if frame.is_line(i) {
                    k[i] >= 1
                } else {
                    k[i] <= 0
                }
            });
            let posnum = (0..n).any(|i| !frame.is_line(i) && k[i] > 0);
            if posnum || (top && !cmp.lower(&k, &key)) {
                descends = false;
            }
            rhs.push(k);
        }
        let mut digest = [0u8; 32];
        if let Some(h) = &r.rhs_sha256 {
            for (i, byte) in digest.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&h[2 * i..2 * i + 2], 16).unwrap_or(0);
            }
        }
        let mut bits = VERDICT_REGULAR;
        if descends {
            bits |= VERDICT_DESCENDS;
        }
        // re-derivation from the verified group (fail closed: a relation that is not verified does not serve)
        let check = vgroup.as_ref().map(|(vg, max_terms)| {
            let rhs_c = coefs.get(&r.ordinal).cloned().unwrap_or_default();
            if rhs_c.iter().map(|(k, _)| k).ne(rhs.iter()) {
                RelationCheck::Unknown("the coefficient list differs from the rhs keys".into())
            } else {
                verify_relation_with(
                    &crate::independent_algebra::Independent,
                    vg,
                    &key,
                    &rhs_c,
                    &[],
                    *max_terms,
                )
            }
        });
        match &check {
            Some(c) if c.verified() => {
                bits |= VERDICT_EXACT;
                rep.relations_verified += 1;
            }
            Some(c) => rep
                .relations_not_verified
                .push((key.clone(), format!("{c:?}"))),
            None if ver.is_some() => bits |= VERDICT_EXACT,
            // a DIAGNOSTIC keys-only relation is followed as if exact (never certified: see check)
            None if src.diagnostic => bits |= VERDICT_EXACT,
            None => {}
        }
        b.push(&key, &rhs, digest, bits)
            .map_err(|e| lerr(format!("{e:?}")))?;
        let label = r.rec.index;
        info.insert(
            key.clone(),
            SuppliedInfo {
                source: src.id.clone(),
                class: class.to_string(),
                path: path.display().to_string(),
                label,
                ordinal: r.ordinal,
                record_sha256: r.sha256.clone(),
                hand_name: src
                    .hand_name
                    .as_ref()
                    .map(|h| hand_name(h, &src.id, None, r, label.unwrap_or(r.ordinal as i64))),
                verification_sha256: ver.clone(),
                check,
                diagnostic: src.diagnostic,
            },
        );
    }
    rep.relations = info.len();
    let relations = b.finish().map_err(|e| ImportError::Source {
        source: src.id.clone(),
        msg: format!("relations: {e:?}"),
    })?;
    // a point relation's domain: the chart only (the certified store decides the key)
    let domain = CaseDomain::free(n, Some(frame.mask().to_vec()));
    Ok((
        RelationSet {
            name: src.id.clone(),
            recipe_digest: format!("sha256:{}", raw.file_sha256),
            domain,
            relations,
            slot: DispatchSlot::Last,
        },
        info,
    ))
}

/// The coefficient texts of every rule record of a supplied file (ordinal -> [(rhs key, coefficient text)]), from
/// the raw `rhs` arrays `[[key, coefficient], ..]` (small files only: the file is read whole).
/// Ordinal -> [(rhs key, coefficient text)] of a supplied file.
type SuppliedCoefficients = HashMap<usize, Vec<(Vec<i32>, String)>>;

fn supplied_coefficients(
    source: &str,
    path: &Path,
    n: usize,
    limits: crate::record::ReadLimits,
) -> Result<SuppliedCoefficients, ImportError> {
    // one line at a time, each at most the per-record budget (lane bigline): a longer line is refused, typed
    let f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let mut rd = std::io::BufReader::with_capacity(1 << 20, f);
    let mut buf: Vec<u8> = vec![];
    let mut out = HashMap::new();
    let mut ordinal = 0usize;
    for i in 0usize.. {
        buf.clear();
        let (got, complete) = crate::record::read_until_limited(
            &mut rd,
            &mut buf,
            limits.record_budget.saturating_add(1),
        )
        .map_err(|e| io_err(path, e))?;
        if got == 0 {
            break;
        }
        if !complete {
            return Err(ImportError::Refused {
                source: source.into(),
                path: path.display().to_string(),
                line: i + 1,
                reason: Box::new(super::error::TableRefusal::RecordTooLarge {
                    record: None,
                    what: "a supplied relation line (its coefficients are kept)".into(),
                    bytes: got as u64,
                    budget: limits.record_budget,
                }),
            });
        }
        let l = std::str::from_utf8(super::lines::line_body(&buf)).map_err(|e| {
            io_err(
                path,
                std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
            )
        })?;
        let l = l.strip_suffix('\r').unwrap_or(l);
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| ImportError::Line {
            source: source.into(),
            path: path.display().to_string(),
            line: i + 1,
            msg: e.to_string(),
        })?;
        if v.get("type").and_then(Value::as_str) != Some("rule") {
            continue;
        }
        let mut terms = vec![];
        for t in v.get("rhs").and_then(Value::as_array).into_iter().flatten() {
            let (k, c) = (
                t.get(0).and_then(Value::as_str),
                t.get(1).and_then(Value::as_str),
            );
            match (k.and_then(|k| parse_key_text(k, n)), c) {
                (Some(k), Some(c)) => terms.push((k, c.to_string())),
                _ => {
                    return Err(ImportError::Line {
                        source: source.into(),
                        path: path.display().to_string(),
                        line: i + 1,
                        msg: "rhs entry is not [key text, coefficient text]".into(),
                    });
                }
            }
        }
        out.insert(ordinal, terms);
        ordinal += 1;
    }
    Ok(out)
}

fn load_recipe(
    src: &SourceSpec,
    class: &str,
    base: &Path,
    frame: &NodeFrame,
) -> Result<RecipeSource, ImportError> {
    let path = resolve(base, src.path.as_deref().unwrap_or_default());
    let d = verify_file(&path, src.sha256.as_deref())?;
    let txt = std::fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
    let v: Value = serde_json::from_str(&txt).map_err(|e| ImportError::Source {
        source: src.id.clone(),
        msg: format!("recipe json: {e}"),
    })?;
    let bad = |m: String| ImportError::Source {
        source: src.id.clone(),
        msg: m,
    };
    if v.get("schema").and_then(Value::as_str) != Some("rustred.recipe.v1") {
        return Err(bad("not a rustred.recipe.v1 file".into()));
    }
    let n = frame.arity();
    if v.get("arity").and_then(Value::as_u64) != Some(n as u64) {
        return Err(bad(format!("recipe arity differs from {n}")));
    }
    if let Some(c) = v.get("chart").and_then(Value::as_str)
        && c != frame.mask_string()
    {
        return Err(bad(format!(
            "recipe chart {c} differs from {}",
            frame.mask_string()
        )));
    }
    let dom = v
        .get("domain")
        .ok_or_else(|| bad("recipe without domain".into()))?;
    let fixed: Vec<Option<i32>> = match dom.get("fixed") {
        None | Some(Value::Null) => vec![None; n],
        Some(Value::Array(a)) if a.len() == n => a
            .iter()
            .map(|x| match x {
                Value::Null => Ok(None),
                x => x
                    .as_i64()
                    .and_then(|v| i32::try_from(v).ok())
                    .map(Some)
                    .ok_or_else(|| bad(format!("fixed entry {x}"))),
            })
            .collect::<Result<_, _>>()?,
        Some(o) => return Err(bad(format!("domain.fixed {o}"))),
    };
    let strs = |x: Option<&Value>| -> Result<Vec<String>, ImportError> {
        match x {
            None | Some(Value::Null) => Ok(vec![]),
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| bad(format!("{e}"))),
        }
    };
    let polys = |v: Vec<String>| -> Result<Vec<Poly>, ImportError> {
        v.iter()
            .map(|s| {
                Poly::parse_d_free_exact(s, n).map_err(|e| bad(format!("polynomial {s}: {e}")))
            })
            .collect()
    };
    let equations = polys(strs(dom.get("equations"))?)?;
    let exc: Vec<Vec<String>> = match dom.get("exceptions") {
        None | Some(Value::Null) => vec![],
        Some(v) => {
            serde_json::from_value(v.clone()).map_err(|e| bad(format!("exceptions: {e}")))?
        }
    };
    if exc.iter().any(Vec::is_empty) {
        return Err(bad("an empty exception AND-list (refused, R45)".into()));
    }
    let exceptions = exc.into_iter().map(polys).collect::<Result<Vec<_>, _>>()?;
    let mut bounds = vec![];
    if let Some(b) = dom.get("bounds").filter(|b| !b.is_null()) {
        match b.as_object() {
            Some(o) if o.keys().all(|k| k == "min" || k == "max") => {}
            _ => return Err(bad(format!("domain.bounds {b} (refused, never ignored)"))),
        }
        for (side, sign) in [("min", 1i32), ("max", -1)] {
            let a = match b.get(side) {
                None | Some(Value::Null) => None,
                Some(Value::Array(a)) => Some(a),
                Some(o) => {
                    return Err(bad(format!(
                        "domain.bounds.{side} {o} (refused, never ignored)"
                    )));
                }
            };
            if let Some(a) = a {
                if a.len() != n {
                    return Err(bad(format!("bounds.{side} length")));
                }
                for (i, x) in a.iter().enumerate() {
                    // exact or refused, never dropped (audit-credits m1: a dropped bound GROWS the domain). JSON
                    // integers in the 64-bit range are exact (i64 / u64); a number beyond them reaches us as a float
                    // whose exact value is lost: the recipe is refused (typed), as is any non-integer entry.
                    let c: i128 = match x {
                        Value::Null => continue,
                        Value::Number(num) => match (num.as_i64(), num.as_u64()) {
                            (Some(c), _) => c as i128,
                            (None, Some(c)) => c as i128,
                            (None, None) => {
                                return Err(bad(format!(
                                    "domain.bounds.{side}[{i}] = {num} is not an exact integer (a bound beyond \
                                     64 bits loses its exact value in JSON: refused, never dropped)"
                                )));
                            }
                        },
                        o => {
                            return Err(bad(format!(
                                "domain.bounds.{side}[{i}] = {o} is not an integer or null (refused, never dropped)"
                            )));
                        }
                    };
                    let p = Poly::slot_minus(i, c);
                    bounds.push(if sign > 0 {
                        p
                    } else {
                        p.neg().map_err(|e| bad(e.to_string()))?
                    });
                }
            }
        }
    }
    // v1.2 per-key assignment `domain.keys` (an explicit certified key set)
    let mut assigned: Option<std::collections::HashSet<Vec<i32>>> = match dom.get("keys") {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => Some(
            a.iter()
                .map(|k| {
                    serde_json::from_value::<Vec<i32>>(k.clone())
                        .ok()
                        .filter(|k| k.len() == n)
                        .ok_or_else(|| bad(format!("domain.keys entry {k}")))
                })
                .collect::<Result<_, _>>()?,
        ),
        Some(o) => return Err(bad(format!("domain.keys {o}"))),
    };
    for k in dom
        .as_object()
        .map(|o| o.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default()
    {
        if !["fixed", "equations", "exceptions", "bounds", "keys"].contains(&k.as_str()) {
            return Err(bad(format!(
                "unknown recipe domain field {k:?} (refused, never ignored)"
            )));
        }
    }
    let recipe_id = v.get("id").and_then(Value::as_str).map(str::to_string);
    if let Some(side) = &src.assignment {
        let p = resolve(base, &side.path);
        verify_file(&p, side.sha256.as_deref())?;
        let txt = std::fs::read_to_string(&p).map_err(|e| io_err(&p, e))?;
        let mut mine: std::collections::HashSet<Vec<i32>> = std::collections::HashSet::new();
        let mut named = false;
        for (i, l) in txt.lines().enumerate() {
            if l.trim().is_empty() {
                continue;
            }
            let e: Value = serde_json::from_str(l).map_err(|er| ImportError::Line {
                source: src.id.clone(),
                path: p.display().to_string(),
                line: i + 1,
                msg: er.to_string(),
            })?;
            let rid = e.get("recipe").and_then(Value::as_str);
            if rid.is_none() || rid != recipe_id.as_deref() {
                continue;
            }
            named = true;
            let mut keys: Vec<&Value> = e
                .get("keys")
                .and_then(Value::as_array)
                .map(|a| a.iter().collect())
                .unwrap_or_default();
            if let Some(k) = e.get("key") {
                keys.push(k);
            }
            for k in keys {
                let key = match k {
                    Value::String(t) => parse_key_text(t, n),
                    _ => serde_json::from_value::<Vec<i32>>(k.clone())
                        .ok()
                        .filter(|k| k.len() == n),
                }
                .ok_or_else(|| ImportError::Line {
                    source: src.id.clone(),
                    path: p.display().to_string(),
                    line: i + 1,
                    msg: format!("assignment key {k}"),
                })?;
                mine.insert(key);
            }
        }
        if !named {
            return Err(bad(format!(
                "the assignment file {} names no key for recipe id {:?}",
                p.display(),
                recipe_id
            )));
        }
        // both given: the intersection (a key must be assigned by both)
        assigned = Some(match assigned {
            Some(a) => a.intersection(&mine).cloned().collect(),
            None => mine,
        });
    }
    let domain = CaseDomain {
        arity: n,
        chart: Some(frame.mask().to_vec()),
        fixed,
        target: TargetMap::Canonical,
        equations,
        bounds,
        exceptions,
        recenter_bounds: vec![],
        poles: vec![],
        witness_elided: false,
    };
    domain
        .validate()
        .map_err(|e| bad(format!("recipe domain: {e}")))?;
    Ok(RecipeSource {
        id: src.id.clone(),
        class: class.to_string(),
        path: path.display().to_string(),
        recipe_sha256: d,
        engine_digest: src.engine_digest.clone(),
        recipe_id,
        domain,
        slot: DispatchSlot::Last,
        assigned,
    })
}

fn read_credit_group(path: &Path, lines: &[bool]) -> Result<CreditPolicy, ImportError> {
    let txt = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let v: Value = serde_json::from_str(&txt).map_err(|e| ImportError::Source {
        source: "symmetry_credit".into(),
        msg: format!("{}: {e}", path.display()),
    })?;
    if v.get("schema").and_then(Value::as_str) != Some("rustred.campaign.credit-group.v1") {
        return Err(ImportError::Unsupported(format!(
            "{}: only rustred.campaign.credit-group.v1 group files are read",
            path.display()
        )));
    }
    let els: Vec<Value> = v
        .get("elements")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut group = vec![];
    for e in els {
        let lines_v: Vec<usize> =
            serde_json::from_value(e.get("lines").cloned().unwrap_or(Value::Null))
                .map_err(|er| ImportError::Spec(format!("credit group element lines: {er}")))?;
        let nums: Option<Vec<usize>> = match e.get("numerators") {
            None | Some(Value::Null) => None,
            Some(x) => Some(
                serde_json::from_value(x.clone())
                    .map_err(|er| ImportError::Spec(format!("credit group numerators: {er}")))?,
            ),
        };
        group.push(GroupElement {
            lines: lines_v,
            numerators: nums,
        });
    }
    let mut pure = BTreeSet::new();
    if let Some(p) = v.get("pure_numerator_maps").and_then(Value::as_array) {
        for x in p {
            pure.insert(
                x.as_u64()
                    .ok_or_else(|| ImportError::Spec("pure_numerator_maps entry".into()))?
                    as usize,
            );
        }
    }
    // a hand-written group file is UNVERIFIED (closurefix): credits from it make the closure `diagnostic`; a verified
    // group (`CreditPolicy::from_verified`) is not an import input in v1
    let pol = CreditPolicy::unverified(group, pure, &format!("import:{}", path.display()));
    pol.validate(lines).map_err(ImportError::Spec)?;
    Ok(pol)
}

/// Read a key list: a JSON array of `I(..)` texts or key arrays, or JSONL of keys (`[..]`, `{"key":[..]}`, `"I(..)"`).
pub fn read_key_list(path: &Path, arity: usize) -> Result<Vec<Vec<i32>>, ImportError> {
    let txt = std::fs::read_to_string(path).map_err(|e| io_err(path, e))?;
    let one = |v: &Value, line: usize| -> Result<Vec<i32>, ImportError> {
        let bad = || ImportError::Evidence {
            path: path.display().to_string(),
            line,
            msg: format!("not a key of arity {arity}: {v}"),
        };
        match v {
            Value::String(s) => parse_key_text(s, arity).ok_or_else(bad),
            Value::Array(_) => serde_json::from_value::<Vec<i32>>(v.clone())
                .ok()
                .filter(|k| k.len() == arity)
                .ok_or_else(bad),
            Value::Object(o) => match o.get("key") {
                Some(Value::String(t)) => parse_key_text(t, arity).ok_or_else(bad),
                Some(k) => serde_json::from_value::<Vec<i32>>(k.clone())
                    .ok()
                    .filter(|k| k.len() == arity)
                    .ok_or_else(bad),
                None => Err(bad()),
            },
            _ => Err(bad()),
        }
    };
    if let Ok(Value::Array(a)) = serde_json::from_str::<Value>(&txt)
        && a.first().is_none_or(|x| !x.is_number())
    {
        return a.iter().map(|v| one(v, 1)).collect();
    }
    let mut out = vec![];
    for (i, l) in txt.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let v: Value = serde_json::from_str(l).map_err(|e| ImportError::Evidence {
            path: path.display().to_string(),
            line: i + 1,
            msg: e.to_string(),
        })?;
        out.push(one(&v, i + 1)?);
    }
    Ok(out)
}

/// The start set of a spec (lane t266run: also used for one clause of a box made of several clauses).
pub fn read_start(
    spec: &TableSetSpec,
    base: &Path,
    codec: KeyCodec,
) -> Result<KeySet, ImportError> {
    let path = resolve(base, &spec.start.path);
    verify_file(&path, spec.start.sha256.as_deref())?;
    let arity = codec.arity();
    let keys: Vec<Vec<i32>> = match spec.start.format {
        StartFormat::KeysJsonl => read_key_list(&path, arity)?,
        StartFormat::KeysetV1 => {
            let f = std::fs::File::open(&path).map_err(|e| io_err(&path, e))?;
            let ks = KeySet::read(std::io::BufReader::new(f))
                .map_err(|e| ImportError::Core(format!("{}: {e:?}", path.display())))?;
            return ks
                .recode(codec)
                .map_err(|e| ImportError::Core(format!("{e:?}")));
        }
        StartFormat::Box => {
            let txt = std::fs::read_to_string(&path).map_err(|e| io_err(&path, e))?;
            let bx = crate::boxdef::BoxDef::from_json(&txt)
                .map_err(|e| ImportError::Spec(format!("{}: box: {e:?}", path.display())))?;
            if bx.arity() != arity {
                return Err(ImportError::Spec(format!(
                    "{}: box of arity {} for a frame of arity {arity}",
                    path.display(),
                    bx.arity()
                )));
            }
            let mut b = KeySetBuilder::new(codec);
            let mut err = None;
            bx.for_each_key(|k| {
                if err.is_none()
                    && let Err(e) = b.push(k)
                {
                    err = Some(format!("box key {k:?}: {e:?}"));
                }
            });
            if let Some(e) = err {
                return Err(ImportError::Core(e));
            }
            return Ok(b.finish());
        }
        StartFormat::HandStatement => {
            let fmt = spec
                .start
                .hand_format
                .as_ref()
                .ok_or_else(|| ImportError::Spec("hand_format".into()))?;
            let h = super::statement::read_hand_statement(&path, fmt, arity)?;
            h.rows
                .iter()
                .filter(|r| r.in_box == Some(true))
                .map(|r| r.key.clone())
                .collect()
        }
    };
    let mut b = KeySetBuilder::new(codec);
    for k in &keys {
        b.push(k)
            .map_err(|e| ImportError::Core(format!("start key {k:?}: {e:?}")))?;
    }
    Ok(b.finish())
}
