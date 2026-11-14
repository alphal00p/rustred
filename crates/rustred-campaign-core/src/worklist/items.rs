//! Faces, cases, items, item ids and node parameters; item construction from a partition report or from the
//! closure's uncovered keys (R30 fallback); the box-restricted constructor that refuses without a user box (R35).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use super::affine::{AffineEq, AffineError, canonical_system};
use super::policy::SeedPolicy;

pub type Key = Vec<i32>;

/// A face (Sd, Sn): the dotted lines (>= 2) and the off-floor numerators (<= -1); sorted, disjoint.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Face {
    pub sd: Vec<usize>,
    pub sn: Vec<usize>,
}

impl Face {
    pub fn new(mut sd: Vec<usize>, mut sn: Vec<usize>) -> Self {
        sd.sort_unstable();
        sd.dedup();
        sn.sort_unstable();
        sn.dedup();
        Face { sd, sn }
    }

    /// The face of a top-sector chart key (every line >= 1, every numerator <= 0); `None` otherwise.
    pub fn of_key(key: &[i32], lines: &[bool]) -> Option<Face> {
        if key.len() != lines.len() {
            return None;
        }
        let mut sd = Vec::new();
        let mut sn = Vec::new();
        for (i, (&v, &is_line)) in key.iter().zip(lines).enumerate() {
            if is_line {
                if v < 1 {
                    return None;
                }
                if v >= 2 {
                    sd.push(i);
                }
            } else {
                if v > 0 {
                    return None;
                }
                if v <= -1 {
                    sn.push(i);
                }
            }
        }
        Some(Face { sd, sn })
    }

    pub fn slots(&self) -> Vec<usize> {
        let mut v: Vec<usize> = self.sd.iter().chain(&self.sn).copied().collect();
        v.sort_unstable();
        v
    }

    pub fn n_slots(&self) -> usize {
        self.sd.len() + self.sn.len()
    }

    /// "{6}x{11}" in presented slots.
    pub fn label(&self) -> String {
        let j = |v: &[usize]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(",");
        format!("{{{}}}x{{{}}}", j(&self.sd), j(&self.sn))
    }
}

/// Floor value of a slot: 1 for a line, 0 for a numerator.
pub fn floor_of(is_line: bool) -> i32 {
    if is_line { 1 } else { 0 }
}

/// One engine case: the face's slots free unless fixed, every other slot at its floor, canonical equations over the
/// free slots (see [`canonical_system`]).
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CaseSpec {
    pub face: Face,
    pub fixed: BTreeMap<usize, i32>,
    pub equations: Vec<AffineEq>,
}

impl CaseSpec {
    /// The FACE CASE (plan section 1): every slot of the face free, the others at their floor.
    pub fn face_case(face: Face) -> Self {
        CaseSpec {
            face,
            fixed: BTreeMap::new(),
            equations: vec![],
        }
    }

    /// A canonical case: fixed values of face slots and equations; equations reduced, unit single-slot equations
    /// turned into fixed values; values of slots outside the face are refused (they are at their floor).
    pub fn new(face: Face, fixed: BTreeMap<usize, i32>, equations: Vec<AffineEq>) -> Result<Self, AffineError> {
        let (equations, fixed) = canonical_system(&equations, &fixed)?;
        Ok(CaseSpec {
            face,
            fixed,
            equations,
        })
    }

    /// This case with more constraints (canonical).
    pub fn constrained(&self, more_fixed: &[(usize, i32)], more_eqs: &[AffineEq]) -> Result<Self, AffineError> {
        let mut fixed = self.fixed.clone();
        for &(s, v) in more_fixed {
            if let Some(&old) = fixed.get(&s)
                && old != v
            {
                return Err(AffineError::Inconsistent);
            }
            fixed.insert(s, v);
        }
        let mut eqs = self.equations.clone();
        eqs.extend(more_eqs.iter().cloned());
        CaseSpec::new(self.face.clone(), fixed, eqs)
    }

    /// Free slots (face slots not fixed), ascending.
    pub fn free(&self) -> Vec<usize> {
        self.face.slots().into_iter().filter(|s| !self.fixed.contains_key(s)).collect()
    }

    /// Free dimension: free slots minus the rank of the (canonical, hence independent) equations.
    pub fn dim(&self) -> u32 {
        (self.free().len().saturating_sub(self.equations.len())) as u32
    }

    pub fn is_point(&self) -> bool {
        self.dim() == 0
    }

    /// Key membership of the case (the engine's case semantics: slots outside the face at their floor, free slots
    /// in the chart orthant, fixed slots equal, equations zero).
    pub fn contains(&self, key: &[i32], lines: &[bool]) -> bool {
        if key.len() != lines.len() {
            return false;
        }
        let face: BTreeSet<usize> = self.face.slots().into_iter().collect();
        for (i, &v) in key.iter().enumerate() {
            if let Some(&f) = self.fixed.get(&i) {
                if v != f {
                    return false;
                }
            } else if face.contains(&i) {
                if (lines[i] && v < 1) || (!lines[i] && v > 0) {
                    return false;
                }
            } else if v != floor_of(lines[i]) {
                return false;
            }
        }
        self.equations.iter().all(|e| e.holds(key))
    }

    /// Off-floor fixed slots (a child's fixed values that are not the floor).
    pub fn fixed_off_floor(&self, lines: &[bool]) -> Vec<usize> {
        self.fixed
            .iter()
            .filter(|(s, v)| **v != floor_of(lines[**s]))
            .map(|(s, _)| *s)
            .collect()
    }

    /// The presented point of a point case (dim 0, no equations) at the floor elsewhere.
    pub fn point_key(&self, lines: &[bool]) -> Option<Key> {
        if !self.equations.is_empty() || !self.free().is_empty() {
            return None;
        }
        Some(
            (0..lines.len())
                .map(|i| self.fixed.get(&i).copied().unwrap_or(floor_of(lines[i])))
                .collect(),
        )
    }

    /// Canonical text used in ids and records: "face={0,6}x{11};fixed=0:2;eq=n6+2*n11-1".
    pub fn canonical_text(&self) -> String {
        let fx: Vec<String> = self.fixed.iter().map(|(s, v)| format!("{s}:{v}")).collect();
        let eq: Vec<String> = self.equations.iter().map(|e| e.to_engine()).collect();
        format!("face={};fixed={};eq={}", self.face.label(), fx.join(","), eq.join(","))
    }

    /// Engine case spec (presented slots): per slot "n<i>" (free) or its value, as the hand campaign's
    /// `--initial-presented` case JSON `{"fixed":[null,1,..],"equations":[..]}`.
    pub fn engine_case_json(&self, lines: &[bool]) -> serde_json::Value {
        let free: BTreeSet<usize> = self.free().into_iter().collect();
        let fixed: Vec<serde_json::Value> = (0..lines.len())
            .map(|i| {
                if free.contains(&i) {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(self.fixed.get(&i).copied().unwrap_or(floor_of(lines[i])))
                }
            })
            .collect();
        serde_json::json!({
            "fixed": fixed,
            "equations": self.equations.iter().map(|e| format!("{}=0", e.to_engine())).collect::<Vec<_>>(),
        })
    }
}

/// Kind of a work item (plan 2.4.3; R35: box-restricted items only with a user box).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ItemKind {
    GenericFace,
    Child,
    Locus,
    Point,
    Phase1Residue,
    BoxRestricted,
    /// OPTIMISE MODE (U-13): find a replacement for the rules `Item::replaces` on this case with a containing domain
    /// and a strictly better economy measure; else the old rules stay.
    Optimise,
}

/// An existing rule an optimise item tries to replace (its measures; containment is the driver's verdict).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OldRule {
    pub digest: String,
    pub rhs_terms: u64,
    pub monomials: Option<u64>,
    pub max_degree: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RootSource {
    Partition,
    ClosureKeys,
    Phase1,
    Repair,
}

/// How an exception-derived child's equations were obtained (R32: read from the rule, or a marked fit).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Derived {
    FromRule,
    Fit,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChildOrigin {
    Root(RootSource),
    /// A standard plane (a dotted line at 2 or 3, a numerator at -1 or -2: R13 A5, R39).
    Standard { slot: usize, value: i32 },
    /// Conjunction of exception branches of accepted rules (`branches`: "rule digest#branch").
    ExceptionBranch { derived: Derived, branches: Vec<String> },
    /// Open obligation of an engine run (`F.obligations.json`, after WP-C0).
    Obligation,
    /// Left-over point of the parent.
    PointOf,
}

/// Stable item id: "it1-" + 32 hex digits of FNV-1a 128 (id version 1) over the canonical item description.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ItemId(pub String);

impl fmt::Display for ItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// FNV-1a, 128 bits (content-addressing only; not cryptographic; a version constant, R: "simple versioning").
pub fn fnv1a128(bytes: &[u8]) -> u128 {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for &b in bytes {
        h ^= b as u128;
        h = h.wrapping_mul(0x0000000001000000000000000000013B);
    }
    h
}

/// One unit of work: a case of one node with its acceptance targets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub id: ItemId,
    pub node: String,
    pub kind: ItemKind,
    pub case: CaseSpec,
    /// Acceptance targets: uncovered keys of the case (box + envelope + closure points, or the repaired key).
    pub gap_points: Vec<Key>,
    pub parent: Option<ItemId>,
    /// Spawn depth (0 for a root item).
    pub depth: u32,
    /// Free dimension of the ROOT item: children never exceed this depth (D-35).
    pub root_dim: u32,
    pub origin: ChildOrigin,
    pub box_id: Option<String>,
    /// Scheduling weight only (never a done criterion): number of gap points.
    pub weight: u64,
    /// Partition proof label of the root ("exact_partition" | "tested" | "closure_lower_bound").
    pub proof: Option<String>,
    /// Optimise items only: the rules to replace.
    #[serde(default)]
    pub replaces: Vec<OldRule>,
    /// MEASURED or PROVED target range (P-2), inherited by children.
    #[serde(default)]
    pub range: RangeLabel,
}

impl Item {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        node: &str,
        kind: ItemKind,
        case: CaseSpec,
        mut gap_points: Vec<Key>,
        parent: Option<ItemId>,
        depth: u32,
        root_dim: u32,
        origin: ChildOrigin,
        box_id: Option<String>,
    ) -> Self {
        gap_points.sort();
        gap_points.dedup();
        let id = item_id(node, kind, &case, box_id.as_deref());
        Item {
            id,
            node: node.to_string(),
            kind,
            weight: gap_points.len() as u64,
            case,
            gap_points,
            parent,
            depth,
            root_dim,
            origin,
            box_id,
            proof: None,
            replaces: Vec::new(),
            range: RangeLabel::Unspecified,
        }
    }

    /// An optimise item (U-13): case = the old rules' common case (face or locus), gap points = sample keys of their
    /// served set (acceptance targets), `replaces` = the old rules.
    pub fn optimise(node: &str, case: CaseSpec, sample_keys: Vec<Key>, replaces: Vec<OldRule>) -> Self {
        let dim = case.dim();
        let mut it = Item::new(node, ItemKind::Optimise, case, sample_keys, None, 0, dim, ChildOrigin::Root(RootSource::Partition), None);
        let mut d: Vec<&str> = replaces.iter().map(|r| r.digest.as_str()).collect();
        d.sort_unstable();
        it.id = ItemId(format!("it1-{:032x}", fnv1a128(format!("v1|optimise|{}|{}|{}", node, it.case.canonical_text(), d.join(",")).as_bytes())));
        it.replaces = replaces;
        it
    }
}

/// The id of an item: content hash of (node, kind class, canonical case, box id). Points of the same case get the
/// same id whatever produced them (a standard child and an exception-derived child on the same plane coincide).
pub fn item_id(node: &str, kind: ItemKind, case: &CaseSpec, box_id: Option<&str>) -> ItemId {
    let class = match kind {
        ItemKind::BoxRestricted => "box",
        ItemKind::Phase1Residue => "p1",
        _ => "case",
    };
    let text = format!(
        "v1|node={node}|class={class}|{}|box={}",
        case.canonical_text(),
        box_id.unwrap_or("")
    );
    ItemId(format!("it1-{:032x}", fnv1a128(text.as_bytes())))
}

/// A box given by the USER in the spec (R35); the tool never chooses or widens one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserBox {
    pub id: String,
    pub given_by: String,
}

/// Master counts of the node (IBP only, with symmetry) and their source (2.4.9, R25).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
/// Optional REFERENCE master counts (U-9: never an input that steers the ladder or decides acceptance; shown only as
/// a comparison column `reference: N` beside `campaign: M`).
pub struct MasterCounts {
    pub ibp: Option<u32>,
    pub with_symmetry: Option<u32>,
    pub source: String,
}

/// Rung budgets (plan 2.10; R34 (d) scaling; R38 oversized class). Seconds and megabytes.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budgets {
    pub base_seconds_d2: u64,
    pub base_seconds_d3: u64,
    pub cap_seconds: u64,
    pub mem_mb_d2: u64,
    pub mem_mb_d3: u64,
    pub point_seconds: u64,
    pub point_mem_mb: u64,
    pub oversized_seconds: u64,
    pub oversized_mem_mb: u64,
    /// Extra CPU allowed after the first acceptable one-case answer, as a factor of its CPU (R31 (b)).
    pub economy_extra_factor: f64,
    /// Re-runs of a time-decided outcome (default 0: move on, marked time_decided).
    pub time_retries: u32,
    pub infra_retries: u32,
    /// Seed count at which an attempt gets its base budget; above it the budget scales linearly (R34 (d); default
    /// learned on the reference: the depth-3 shell of a single-dot face, 1,771 seeds).
    pub scale_reference_seeds: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Budgets {
            base_seconds_d2: 300,
            base_seconds_d3: 1800,
            cap_seconds: 7200,
            mem_mb_d2: 2048,
            mem_mb_d3: 8192,
            point_seconds: 1800,
            point_mem_mb: 5120,
            oversized_seconds: 3600,
            oversized_mem_mb: 16384,
            economy_extra_factor: 2.0,
            time_retries: 0,
            infra_retries: 10,
            scale_reference_seeds: 1771,
        }
    }
}

/// Acceptance / evidence levels A0-A4 (plan section 1, D-6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum EvidenceLevel {
    A0,
    A1,
    A2,
    A3,
    A4,
}

/// Parameters of one node (all spec fields, no code constants: H s.6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct NodeParams {
    pub node: String,
    pub arity: usize,
    /// `lines[i]` = slot i is a line (presented frame).
    pub lines: Vec<bool>,
    pub node_class: String,
    pub user_box: Option<UserBox>,
    pub budgets: Budgets,
    pub policy: SeedPolicy,
    /// Required evidence level, a per-node SPEC value (U-12): `replay` = A2 (identity replay at two primes distinct
    /// from the reconstruction primes + descent + guard / pole checks), `per-key` = A3. Never hard-wired.
    pub min_evidence: EvidenceLevel,
    pub terminals_max: usize,
    /// Optional reference only (U-9).
    pub reference_masters: MasterCounts,
    pub enable_freeze_rung: bool,
    pub max_item_attempts: u32,
    /// IBP rows per seed of the family (spec / family data; 0 = unknown, no work counter).
    pub ibp_rows_per_seed: u32,
    pub standard_values: StandardValues,
    /// PREFERRED terminals of the spec (P-4; interfaces/preferred-terminals.md): a preference, never a restriction.
    pub preferred_terminals: Vec<Key>,
    /// Whether the key sets this node closes against are a MEASURED or a PROVED range (P-2); carried into items.
    pub target_range: RangeLabel,
}

/// Provenance of a target key set (R41 P-2): closure against a MEASURED hand-down range is provisional (the caller
/// labels it), against a PROVED range final.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RangeLabel {
    Proved,
    Measured,
    #[default]
    Unspecified,
}

impl NodeParams {
    /// Defaults of plan 2.10 for a node with `n_lines` lines first (lines are slots 0..n_lines).
    pub fn new(node: &str, arity: usize, n_lines: usize) -> Self {
        Self::with_lines(node, (0..arity).map(|i| i < n_lines).collect())
    }

    /// Defaults for a node with an arbitrary line mask (`lines[i]` = slot i is a line).
    pub fn with_lines(node: &str, lines: Vec<bool>) -> Self {
        NodeParams {
            node: node.to_string(),
            arity: lines.len(),
            lines,
            node_class: "*".into(),
            user_box: None,
            budgets: Budgets::default(),
            policy: SeedPolicy::builtin(),
            min_evidence: EvidenceLevel::A2,
            terminals_max: 100,
            reference_masters: MasterCounts::default(),
            enable_freeze_rung: false,
            max_item_attempts: 12,
            ibp_rows_per_seed: 0,
            standard_values: StandardValues::default(),
            preferred_terminals: Vec::new(),
            target_range: RangeLabel::Unspecified,
        }
    }
}

/// Errors of item construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorklistError {
    /// R35: a box-restricted item needs a box GIVEN BY THE USER in the node's spec.
    NoUserBox,
    /// The node's box is not marked `given_by: "user"`.
    BoxNotUserGiven(String),
    /// A key is not a top-sector chart key of the node (wrong arity, a removed line or a positive numerator).
    NotTopChart(Key),
    Case(String),
}

impl fmt::Display for WorklistError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorklistError::NoUserBox => write!(f, "box-restricted item refused: the node's spec has no user box (R35)"),
            WorklistError::BoxNotUserGiven(g) => write!(f, "box given by {g:?}, not by the user (R35)"),
            WorklistError::NotTopChart(k) => write!(f, "not a top-sector chart key: {k:?}"),
            WorklistError::Case(s) => write!(f, "case: {s}"),
        }
    }
}

impl std::error::Error for WorklistError {}

/// One locus of a partition face (equations over the face's slots; affine only: nonlinear loci are UNRESOLVED and
/// come as plain points).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocusInput {
    pub equations: Vec<AffineEq>,
    pub points: Vec<Key>,
    pub proof: String,
}

/// One gap face of a partition report (the partition lane's output, re-expressed as plain data).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GapFaceInput {
    pub face: Face,
    /// The generic class is uncovered (else: locus-only face).
    pub generic: bool,
    /// Uncovered census / closure points of the face that lie on none of its loci.
    pub points: Vec<Key>,
    pub loci: Vec<LocusInput>,
    pub out_of_box: bool,
    pub proof: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PartitionReport {
    pub faces: Vec<GapFaceInput>,
}

/// Items of a node built from a partition report: one GENERIC FACE item per generic face (whole face, all powers and
/// ranks: R35), one LOCUS item per locus; out-of-box faces are returned separately (fail-closed, not queued).
pub fn items_from_partition(
    report: &PartitionReport,
    params: &NodeParams,
) -> Result<(Vec<Item>, Vec<Face>), WorklistError> {
    let mut items = Vec::new();
    let mut out_of_box = Vec::new();
    for f in &report.faces {
        if f.out_of_box {
            out_of_box.push(f.face.clone());
            continue;
        }
        let case = CaseSpec::face_case(f.face.clone());
        let dim = case.dim();
        if f.generic || !f.points.is_empty() {
            let mut it = Item::new(
                &params.node,
                ItemKind::GenericFace,
                case.clone(),
                f.points.clone(),
                None,
                0,
                dim,
                ChildOrigin::Root(RootSource::Partition),
                None,
            );
            it.proof = Some(f.proof.clone());
            items.push(it);
        }
        for l in &f.loci {
            let lc = case
                .constrained(&[], &l.equations)
                .map_err(|e| WorklistError::Case(e.to_string()))?;
            let kind = if lc.is_point() { ItemKind::Point } else { ItemKind::Locus };
            let mut it = Item::new(
                &params.node,
                kind,
                lc,
                l.points.clone(),
                None,
                0,
                dim,
                ChildOrigin::Root(RootSource::Partition),
                None,
            );
            it.proof = Some(l.proof.clone());
            items.push(it);
        }
    }
    order_items(&mut items);
    Ok((items, out_of_box))
}

/// Items from the closure's uncovered keys (R30 fallback when the partition does not scale): one whole-face item per
/// face spanned by the keys; the proof label is `closure_lower_bound`.
pub fn items_from_closure_keys(keys: &[Key], params: &NodeParams) -> Result<Vec<Item>, WorklistError> {
    let mut by_face: BTreeMap<Face, Vec<Key>> = BTreeMap::new();
    for k in keys {
        let f = Face::of_key(k, &params.lines).ok_or_else(|| WorklistError::NotTopChart(k.clone()))?;
        by_face.entry(f).or_default().push(k.clone());
    }
    let mut items: Vec<Item> = by_face
        .into_iter()
        .map(|(f, pts)| {
            let case = CaseSpec::face_case(f);
            let dim = case.dim();
            let kind = if dim == 0 { ItemKind::Point } else { ItemKind::GenericFace };
            let mut it = Item::new(
                &params.node,
                kind,
                case,
                pts,
                None,
                0,
                dim,
                ChildOrigin::Root(RootSource::ClosureKeys),
                None,
            );
            it.proof = Some("closure_lower_bound".into());
            it
        })
        .collect();
    order_items(&mut items);
    Ok(items)
}

/// A box-restricted item (the BOXED fallback of 2.4.15): only for a node whose spec carries a user box.
pub fn box_restricted_item(case: CaseSpec, gap_points: Vec<Key>, params: &NodeParams) -> Result<Item, WorklistError> {
    let b = params.user_box.as_ref().ok_or(WorklistError::NoUserBox)?;
    if b.given_by != "user" {
        return Err(WorklistError::BoxNotUserGiven(b.given_by.clone()));
    }
    let dim = case.dim();
    Ok(Item::new(
        &params.node,
        ItemKind::BoxRestricted,
        case,
        gap_points,
        None,
        0,
        dim,
        ChildOrigin::Root(RootSource::Partition),
        Some(b.id.clone()),
    ))
}

/// The item for one key in live repair: the WHOLE face case of the key, gap points = the key and extra targets.
pub fn repair_item(key: &[i32], extra: &[Key], params: &NodeParams) -> Result<Item, WorklistError> {
    let f = Face::of_key(key, &params.lines).ok_or_else(|| WorklistError::NotTopChart(key.to_vec()))?;
    let case = CaseSpec::face_case(f);
    let mut pts = vec![key.to_vec()];
    pts.extend(extra.iter().filter(|k| case.contains(k, &params.lines)).cloned());
    let dim = case.dim();
    let kind = if dim == 0 { ItemKind::Point } else { ItemKind::GenericFace };
    Ok(Item::new(&params.node, kind, case, pts, None, 0, dim, ChildOrigin::Root(RootSource::Repair), None))
}

/// Scheduling order, a HEURISTIC only (DESIGN-COMMENTS A3): weight descending, then pour order (more face slots
/// first, more numerators first), then id. Never a correctness assumption.
pub fn order_items(items: &mut [Item]) {
    items.sort_by(|a, b| {
        b.weight
            .cmp(&a.weight)
            .then(b.case.face.n_slots().cmp(&a.case.face.n_slots()))
            .then(b.case.face.sn.len().cmp(&a.case.face.sn.len()))
            .then(a.id.cmp(&b.id))
    });
}

/// Standard plane values (DATA, spec field `standard_values`; default learned from the evidence so far: R13 A5
/// "line at 2, numerator at -1", extended by R39 to line 3 and numerator -2). Fixed values READ from a rule's
/// exceptions take precedence over this list (children.rs).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StandardValues {
    pub line: Vec<i32>,
    pub numerator: Vec<i32>,
}

impl Default for StandardValues {
    fn default() -> Self {
        StandardValues {
            line: vec![2, 3],
            numerator: vec![-1, -2],
        }
    }
}

impl StandardValues {
    pub fn for_slot(&self, is_line: bool) -> &[i32] {
        if is_line { &self.line } else { &self.numerator }
    }
}

/// The standard children of an item, created WITH it in state `blocked: parent_pending` (2.4.4): one per free slot
/// and value of the node's [`StandardValues`]; each carries the item's gap points on its plane (possibly none).
pub fn standard_children(item: &Item, params: &NodeParams) -> Vec<Item> {
    let mut out = Vec::new();
    if item.case.dim() == 0 {
        return out;
    }
    for s in item.case.free() {
        for &v in params.standard_values.for_slot(params.lines[s]) {
            let Ok(c) = item.case.constrained(&[(s, v)], &[]) else { continue };
            if c.dim() >= item.case.dim() {
                continue;
            }
            let pts: Vec<Key> = item.gap_points.iter().filter(|k| c.contains(k, &params.lines)).cloned().collect();
            let kind = if c.is_point() { ItemKind::Point } else { ItemKind::Child };
            out.push(Item::new(
                &params.node,
                kind,
                c,
                pts,
                Some(item.id.clone()),
                item.depth + 1,
                item.root_dim,
                ChildOrigin::Standard { slot: s, value: v },
                item.box_id.clone(),
            ));
            if let Some(last) = out.last_mut() {
                last.range = item.range;
            }
        }
    }
    out
}
