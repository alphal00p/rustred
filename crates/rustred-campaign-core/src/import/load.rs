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
use super::lines::{RawRecord, RawTable, parse_key_text, read_sha256sums, scan_table, verify_file};
use super::spec::{
    DomainMode, ElidedPolicy, HandName, SourceKind, SourceSpec, StartFormat, TableSetSpec, resolve,
};
use crate::closure::relations::{
    DispatchSlot, RelationSet, RelationStoreBuilder, VERDICT_DESCENDS, VERDICT_EXACT,
    VERDICT_REGULAR,
};
use crate::closure::{KeyCodec, KeySet, KeySetBuilder, RuleBodies};
use crate::cover::{Dispatcher, Table, WitnessStatus};
use crate::domain::{CaseDomain, DomainOptions, TargetMap};
use crate::key::NodeFrame;
use crate::order::{GroupElement, NodeComparator};
use crate::poly::Poly;
use crate::record::{TableFile, TableHeader};
use crate::symmetry::CreditPolicy;

/// Witness status of an imported record as the dispatch sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessKind {
    Complete,
    NotCarried,
    Elided,
    /// Elided, loaded with the witness check off under `elided: lift_with_per_key_proof`.
    ElidedLifted,
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

/// Load the spec. Relative paths resolve against `base`.
pub fn load(spec: &TableSetSpec, base: &Path) -> Result<ImportedSet, ImportError> {
    let spec_digest = spec.digest();
    // ---- the frame: the first file of `frame_from`
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
    let fhead = scan_table(&fsrc.id, &first.path, false, &|_, _| false)?.header;
    let frame = NodeFrame::from_header(&fhead).map_err(|e| ImportError::Source {
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
    let mut fact_tables = BTreeMap::new();
    let mut table_digests = BTreeMap::new();
    let mut supplied: Vec<Arc<RelationSet>> = vec![];
    let mut supplied_info = BTreeMap::new();
    let mut recipes = vec![];
    let mut terminals_preferred = vec![];
    let mut terminals_other = vec![];
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
                for (ui, u) in units.iter().enumerate() {
                    let want: Option<Vec<i64>> = src.records.clone();
                    let raw = scan_table(&src.id, &u.path, false, &|_, _| true)?;
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
                    rep.files.push((u.path.display().to_string(), sha.clone()));
                    rep.records_in_files += raw.records.len();
                    for (k, n) in &raw.kinds {
                        *rep.line_kinds.entry(k.clone()).or_default() += n;
                    }
                    if !raw.kinds.contains_key("end") {
                        rep.files_without_end.push(u.path.display().to_string());
                    }
                    // labels unique inside the file / source / scope
                    let mut by_label: BTreeMap<i64, usize> = BTreeMap::new();
                    for (i, r) in raw.records.iter().enumerate() {
                        let l = r.rec.i.or(r.rec.index).ok_or_else(|| ImportError::Line {
                            source: src.id.clone(),
                            path: u.path.display().to_string(),
                            line: r.line_no,
                            msg: "rule record without index".into(),
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
                        let l = raw.records[i]
                            .rec
                            .i
                            .or(raw.records[i].rec.index)
                            .unwrap_or(i64::MIN);
                        if let Some(reason) = removed.get(&l) {
                            seen_removed.insert(l);
                            rep.removed.push((l, reason.clone()));
                            false
                        } else {
                            true
                        }
                    });
                    // segments: split around elided records when they are lifted
                    let lift = spec.policies.elided == ElidedPolicy::LiftWithPerKeyProof;
                    let mut segments: Vec<(Vec<usize>, bool)> = vec![];
                    let mut cur: Vec<usize> = vec![];
                    for &i in &order {
                        let el = raw.records[i]
                            .rec
                            .witness_loci
                            .as_ref()
                            .is_some_and(|w| w.iter().any(|s| s.trim() == "elided"));
                        if lift && el {
                            if !cur.is_empty() {
                                segments.push((std::mem::take(&mut cur), false));
                            }
                            segments.push((vec![i], true));
                        } else {
                            cur.push(i);
                        }
                    }
                    if !cur.is_empty() || segments.is_empty() {
                        segments.push((cur, false));
                    }
                    let base_name = match &u.table_label {
                        Some(t) => format!("{}/{}", src.id, t),
                        None => src.id.clone(),
                    };
                    let nseg = segments.len();
                    for (si, (seg, lifted)) in segments.into_iter().enumerate() {
                        let name = if nseg == 1 {
                            base_name.clone()
                        } else if lifted {
                            let l = raw.records[seg[0]].rec.index.unwrap_or(-1);
                            format!("{base_name}~lift{l}")
                        } else {
                            format!("{base_name}~{si}")
                        };
                        if seg.is_empty() {
                            continue;
                        }
                        let (t, tf, infos) = build_table(
                            src, &class, &name, u, &raw, &seg, lifted, &frame, &mut rep,
                        )?;
                        if t.is_empty() {
                            continue;
                        }
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
                    let _ = ui;
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
                        let d = verify_file(&path, src.sha256.as_deref())?;
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
                if src.preferred.unwrap_or(false) {
                    terminals_preferred.extend(keys);
                } else {
                    terminals_other.extend(keys);
                }
            }
            SourceKind::SymmetryCredit => {
                if credits.is_some() {
                    return Err(ImportError::Spec(
                        "more than one symmetry_credit source".into(),
                    ));
                }
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
        let want: BTreeMap<usize, &str> = ts
            .iter()
            .flat_map(|&t| body_plan[t].2.iter().map(|(o, s)| (*o, s.as_str())))
            .collect();
        let src_id = &body_plan[ts[0]].1;
        let raw = scan_table(src_id, path, true, &|o, _| want.contains_key(&o))?;
        let mut by_ord: HashMap<usize, RawRecord> = HashMap::new();
        for r in raw.records {
            if want.get(&r.ordinal) != Some(&r.sha256.as_str()) {
                return Err(ImportError::Source {
                    source: src_id.clone(),
                    msg: format!(
                        "{} changed between the passes (record {})",
                        path.display(),
                        r.ordinal
                    ),
                });
            }
            by_ord.insert(r.ordinal, r);
        }
        for &t in ts {
            let tf = TableFile {
                header: Some(raw.header.clone()),
                rules: body_plan[t]
                    .2
                    .iter()
                    .filter_map(|(o, _)| by_ord.get(o).map(|r| r.rec.clone()))
                    .collect(),
                charts: raw.charts.clone(),
                others: vec![],
            };
            bodies.load_table_records(&disp, t, &tf).map_err(|e| {
                ImportError::Core(format!("bodies of {}: {e:?}", disp.tables()[t].name))
            })?;
        }
    }
    for t in 0..disp.tables().len() {
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

fn check_header(
    source: &str,
    h: &TableHeader,
    frame: &NodeFrame,
    frame_desc: &str,
    fhead: &TableHeader,
) -> Result<(), ImportError> {
    let f = NodeFrame::from_header(h).map_err(|e| ImportError::Source {
        source: source.into(),
        msg: format!("frame: {e}"),
    })?;
    if f.arity() != frame.arity() || !f.same_mask(frame) || f.power_shifts() != frame.power_shifts()
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
    lifted: bool,
    frame: &NodeFrame,
    rep: &mut SourceReport,
) -> Result<(Table, TableFile, Vec<RuleInfo>), ImportError> {
    let tf = TableFile {
        header: Some(raw.header.clone()),
        rules: seg.iter().map(|&i| raw.records[i].rec.clone()).collect(),
        charts: raw.charts.clone(),
        others: vec![],
    };
    let opts = DomainOptions {
        witness: !lifted,
        ..Default::default()
    };
    let mut t =
        Table::from_table_file(name, u.path.display().to_string(), &tf, opts).map_err(|e| {
            ImportError::Source {
                source: src.id.clone(),
                msg: format!("{}: {e}", u.path.display()),
            }
        })?;
    for s in &t.provenance.skipped_not_dispatched {
        rep.not_dispatched
            .push((name.to_string(), s.label, s.dispatch.clone()));
    }
    let mut infos = vec![];
    for k in 0..t.len() {
        let r = &raw.records[seg[t.meta[k].source_index]];
        let label = t.labels[k];
        let mut bounds: Vec<Poly> = vec![];
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
                if let Some(lo) = db.min[i] {
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
        let witness = match (t.meta[k].witness, lifted) {
            (_, true) => WitnessKind::ElidedLifted,
            (WitnessStatus::Complete, _) => WitnessKind::Complete,
            (WitnessStatus::Elided, _) => WitnessKind::Elided,
            (WitnessStatus::NotCarried, _) => WitnessKind::NotCarried,
        };
        *rep.witness.entry(format!("{witness:?}")).or_default() += 1;
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
) -> Result<(RelationSet, BTreeMap<Vec<i32>, SuppliedInfo>), ImportError> {
    let path = resolve(base, src.path.as_deref().unwrap_or_default());
    let raw = scan_table(&src.id, &path, true, &|_, _| true)?;
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
        if ver.is_some() {
            bits |= VERDICT_EXACT;
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
            .map(|s| Poly::parse_d_free(s, n).map_err(|e| bad(format!("polynomial {s}: {e}"))))
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
        for (side, sign) in [("min", 1i32), ("max", -1)] {
            if let Some(a) = b.get(side).and_then(Value::as_array) {
                if a.len() != n {
                    return Err(bad(format!("bounds.{side} length")));
                }
                for (i, x) in a.iter().enumerate() {
                    if let Some(c) = x.as_i64() {
                        let p = Poly::slot_minus(i, c as i128);
                        bounds.push(if sign > 0 {
                            p
                        } else {
                            p.neg().map_err(|e| bad(e.to_string()))?
                        });
                    }
                }
            }
        }
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
        recipe_id: v.get("id").and_then(Value::as_str).map(str::to_string),
        domain,
        slot: DispatchSlot::Last,
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
            Value::Object(o) => o
                .get("key")
                .and_then(|k| serde_json::from_value::<Vec<i32>>(k.clone()).ok())
                .filter(|k| k.len() == arity)
                .ok_or_else(bad),
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

fn read_start(spec: &TableSetSpec, base: &Path, codec: KeyCodec) -> Result<KeySet, ImportError> {
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
        StartFormat::HandStatement => {
            let fmt = spec
                .start
                .hand_format
                .as_ref()
                .ok_or_else(|| ImportError::Spec("hand_format".into()))?;
            let h = super::statement::read_hand_statement(&path, fmt, arity)?;
            h.rows
                .iter()
                .filter(|r| r.in_box)
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
