//! Forward closure of a key set under the actual first-match dispatch (plan 2.4.8; COORDINATOR_REQUIREMENTS R10,
//! R19, R20; DESIGN-COMMENTS A1, A2, B6). Interface: plan/auto-campaign/interfaces/closure.md.
//!
//! Start from the keys; dispatch each ([`fast::FastDispatch`] = `Dispatcher::first_match`); instantiate the winner's
//! right-hand-side key patterns at the key ([`bodies`]); classify every term (positive numerator FIRST, then zero
//! sector, subsector hand-down, top sector); follow the top-sector terms; repeat to a fixed point (BFS by rounds).
//! Every top edge is checked strictly lower under the node comparator when it is generated; edges are counted, not
//! stored. Open keys (uncovered, blocked, undecided, outside the chart, no body, irregular, unrepresentable) are the
//! closure DEFECT and name their blocking rule. Tier K (keys only) is a labelled SUPERSET closure and never issues
//! `closed`; Tier C consults a [`facts::FactSource`] and drops a term only on a verified zero coefficient.
//!
//! Determinism: keys get indices start-set first (sorted), then per round in sorted code order; per-round work is
//! split into contiguous chunks and merged in chunk order; witnesses are the smallest by (key index, term). The
//! result does not depend on the thread count.
//!
//! Memory (per reached key): packed code (8 bytes per word), table slot (<= 2 x 4 bytes), result (4 bytes),
//! stopping position (4 bytes); edges are not stored. The longest chain and the cycle check run a memoised
//! depth-first search that recomputes successors (one more instantiation pass; memory: 5 bytes per key + stack).

pub mod bodies;
pub mod exits;
pub mod facts;
pub mod fast;
pub mod keyset;
pub mod sha256;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::cover::{CoverError, Dispatch, Dispatcher, RuleRef};
use crate::order::NodeComparator;
use crate::symmetry::{Credit, CreditPolicy, Refusal};

pub use bodies::{
    BodyError, BodyLoadStats, RhsSlot, RuleBodies, Sectors, TermClass, parse_rhs_pattern,
};
pub use facts::{
    CoefFact, FactError, FactKind, FactRequest, FactSource, FileFacts, NoFacts, RegFact,
    RequestReason, RuleId, write_fact_requests,
};
pub use fast::FastDispatch;
pub use keyset::{KeyCodec, KeySet, KeySetBuilder, KeySetError, KeyTable};

pub const CLOSURE_SCHEMA: &str = "rustred.campaign.closure.v1";

/// Closure tier (plan 2.4.8, Decision D-7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tier {
    /// Keys only: superset closure, never `closed`.
    K,
    /// With coefficient / regularity facts.
    C,
}

/// What to keep of the hand-downs (subsector right-hand-side keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HandDowns {
    Off,
    /// Term counts per child sector.
    Counts,
    /// Counts and the distinct keys per child sector.
    Keys,
}

/// Configuration of a closure run.
#[derive(Clone, Debug)]
pub struct ClosureConfig {
    pub tier: Tier,
    /// Worker threads (never changes a result).
    pub threads: usize,
    /// Stop after this many rounds (the remaining frontier is reported open as `Truncated`).
    pub max_rounds: Option<u32>,
    /// Bits per slot of the key codec (6: -32..31).
    pub bits: u32,
    pub hand_downs: HandDowns,
    /// Dropped-line masks of zero sectors.
    pub zero_sectors: Option<BTreeSet<u64>>,
    /// Line mask when the dispatcher carries no chart.
    pub lines: Option<Vec<bool>>,
    /// Tier C: an Unknown regularity makes the closure undecided.
    pub require_regularity: bool,
    /// Compute the longest chain and the explicit cycle check.
    pub longest_chain: bool,
    /// Witnesses kept per kind.
    pub witness_cap: usize,
    /// Symmetry credits as a dispatch fallback for uncovered keys.
    pub credits: Option<CreditPolicy>,
    /// Declared terminals (R41 P-4: preferred by the user, and others): checked BEFORE dispatch; a declared terminal
    /// ends the walk at that key (no rule is applied there, even inside a rule's domain) and is counted `terminal`.
    pub terminals: Option<TerminalSets>,
    /// Labels of the inputs (start set, envelope files): `measured` inputs make the verdict provisional (R41 P-2).
    pub inputs: Vec<InputLabel>,
    /// Fact identity of a table (EC-2 `rr_proof instantiate` reads `table` as the PATH of the table_dump.v1 file with
    /// coefficients): table name -> path. Default: the table's `provenance.source`.
    pub fact_tables: BTreeMap<String, String>,
}

/// Whether an input key set is proved or only measured (R41 P-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputProvenance {
    Measured,
    Proved,
}

impl InputLabel {
    fn provisional_input(&self) -> bool {
        self.provenance == InputProvenance::Measured
    }
}

/// One labelled input of a closure run (start key set, an envelope file).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct InputLabel {
    pub name: String,
    pub digest: String,
    pub provenance: InputProvenance,
}

/// Declared terminal keys (key-exclusions from the dispatch): `preferred` = the user's preferred terminals, `other` =
/// any other declared terminals. Keys in both count as preferred.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalSets {
    pub preferred: Option<KeySet>,
    pub other: Option<KeySet>,
}

/// Kind of a declared terminal reached by the closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalKind {
    Preferred,
    Other,
}

/// A declared terminal reached by the closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRecord {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub kind: TerminalKind,
    /// The rule the dispatch would have applied there (None: uncovered or open anyway).
    pub would_dispatch: Option<RuleName>,
}

impl Default for ClosureConfig {
    fn default() -> Self {
        ClosureConfig {
            tier: Tier::K,
            threads: 4,
            max_rounds: None,
            bits: 6,
            hand_downs: HandDowns::Counts,
            zero_sectors: None,
            lines: None,
            require_regularity: true,
            longest_chain: true,
            witness_cap: 20,
            credits: None,
            terminals: None,
            inputs: vec![],
            fact_tables: BTreeMap::new(),
        }
    }
}

/// Errors (caller mistakes and inconsistent inputs; never a silent pass).
#[derive(Debug)]
pub enum ClosureError {
    KeyArity {
        expected: usize,
        got: usize,
    },
    KeySet(KeySetError),
    Cover(CoverError),
    Body(BodyError),
    /// No line mask (dispatcher without chart and none configured).
    NoLines,
    /// The previous dispatch cache does not fit the new dispatcher.
    NotIncremental(String),
}
impl std::fmt::Display for ClosureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for ClosureError {}
impl From<KeySetError> for ClosureError {
    fn from(e: KeySetError) -> Self {
        ClosureError::KeySet(e)
    }
}
impl From<BodyError> for ClosureError {
    fn from(e: BodyError) -> Self {
        ClosureError::Body(e)
    }
}

/// Rule identity in reports.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct RuleName {
    pub table: String,
    pub record: usize,
    pub label: i64,
}

/// Why a reached key is open.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenStatus {
    Uncovered,
    Blocked,
    Undecided,
    OutsideChart,
    NoBody,
    Irregular,
    Truncated,
}

/// One open key of the closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenKey {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub status: OpenStatus,
    pub rule: Option<RuleName>,
    pub detail: String,
}

/// A witness of a failure (descent violation, non-zero positive-numerator term, unrepresentable key).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Witness {
    pub key: Vec<i32>,
    pub rule: RuleName,
    pub term: usize,
    pub rhs: Vec<i32>,
    pub reason: String,
}

/// A symmetry credit used by the closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditRecord {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub element: usize,
    pub image: Vec<i32>,
    pub rule: RuleName,
    pub descent_checked: bool,
}

/// Term statistics.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TermStats {
    pub total: u64,
    pub top: u64,
    pub subsector: u64,
    pub zero_sector: u64,
    pub positive_numerator_top: u64,
    pub positive_numerator_sub: u64,
    pub positive_numerator_symbolic: u64,
    pub positive_numerator_constant: u64,
    pub dropped_zero: u64,
}

impl TermStats {
    fn add(&mut self, o: &TermStats) {
        self.total += o.total;
        self.top += o.top;
        self.subsector += o.subsector;
        self.zero_sector += o.zero_sector;
        self.positive_numerator_top += o.positive_numerator_top;
        self.positive_numerator_sub += o.positive_numerator_sub;
        self.positive_numerator_symbolic += o.positive_numerator_symbolic;
        self.positive_numerator_constant += o.positive_numerator_constant;
        self.dropped_zero += o.dropped_zero;
    }
}

/// Undecided counts (Tier C).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Undecided {
    pub coef_unknown_positive_numerator: u64,
    pub coef_unknown_not_lower: u64,
    pub regularity_unknown: u64,
}

impl Undecided {
    pub fn total(&self) -> u64 {
        self.coef_unknown_positive_numerator + self.coef_unknown_not_lower + self.regularity_unknown
    }
}

/// The closure verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Tier C, everything decided with exact facts, no defect, no failure.
    Closed,
    /// As `Closed`, but some facts are provisional.
    ClosedProvisional,
    /// Tier K: no defect and no failure in the superset (still NOT a closed verdict).
    SupersetClean,
    /// Open keys exist.
    Open,
    /// Tier C: facts missing.
    Undecided,
    /// A descent violation, a cycle, a non-zero positive-numerator term or an unrepresentable key.
    Failed,
}

/// The longest chain (edges) and where it starts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chain {
    pub edges: u64,
    pub from: Vec<i32>,
    pub to: Vec<i32>,
    pub longest_from_start_key: u64,
    pub path_digest: String,
}

/// The numbers of a closure run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureSummary {
    pub tier: Tier,
    pub superset: bool,
    pub verdict: Verdict,
    pub rounds: u32,
    pub truncated: bool,
    pub keys_total: u64,
    pub keys_in_start: u64,
    pub keys_outside: u64,
    pub edges: u64,
    pub terms: TermStats,
    pub requires_coefficient_zero: u64,
    pub descent_violations: u64,
    pub positive_numerator_nonzero: u64,
    pub unrepresentable: u64,
    pub cycles: Option<u64>,
    pub longest_chain: Option<Chain>,
    pub undecided: Undecided,
    pub open_total: u64,
    pub open_in_start: u64,
    pub open_outside: u64,
    pub open_by_status: BTreeMap<String, u64>,
    /// "{dotted lines}x{numerator slots}" -> [in start, outside] (chart keys only).
    pub open_by_face: BTreeMap<String, [u64; 2]>,
    pub credited: u64,
    pub credited_in_start: u64,
    pub used_rules: u64,
    pub used_rules_in_start: u64,
    /// per table name: [wins on all keys, wins on start keys]
    pub wins_per_table: BTreeMap<String, [u64; 2]>,
    /// per child sector (dropped-line mask): [terms, distinct keys (if kept)]
    pub hand_downs: BTreeMap<u64, [u64; 2]>,
    pub fact_requests: u64,
    /// True if any labelled input is `measured` (R41 P-2): a closed verdict is then provisional.
    pub provisional: bool,
    pub inputs: Vec<InputLabel>,
    pub terminals_preferred: u64,
    pub terminals_other: u64,
    pub terminals_in_start: u64,
    pub fact_source: String,
    pub facts_exact: bool,
    pub dispatch_order_digest: String,
    pub start_set_digest: String,
}

/// Identifying fields supplied by the caller for the report artefact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReportIdentity {
    pub node: String,
    pub seq: u64,
    pub table_set_digest: String,
    pub box_digest: Option<String>,
    pub envelope_digest: Option<String>,
    pub tool_versions: BTreeMap<String, String>,
}

/// Per-rule firing counts (R20).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleUse {
    pub rule: RuleName,
    pub fires: u64,
    pub fires_in_start: u64,
}

/// The closure report artefact (`reports/<node>/closure-<seq>.json`, schema [`CLOSURE_SCHEMA`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClosureReport {
    pub schema: String,
    pub identity: ReportIdentity,
    pub config: BTreeMap<String, String>,
    pub summary: ClosureSummary,
    pub descent_witnesses: Vec<Witness>,
    pub failure_witnesses: Vec<Witness>,
    pub open_keys: Vec<OpenKey>,
    pub open_keys_digest: String,
    pub credits: Vec<CreditRecord>,
    pub terminals: Vec<TerminalRecord>,
    pub hand_down_digests: BTreeMap<u64, String>,
    pub used_rules: Vec<RuleUse>,
    pub timing: Option<BTreeMap<String, f64>>,
}

// ------------------------------------------------------------------------------------------------ engine internals

const PENDING: u32 = u32::MAX - 2;
const CREDITED: u32 = u32::MAX - 1;
const OPEN: u32 = u32::MAX;
const TERMINAL: u32 = u32::MAX - 3;
const NO_STOP: u32 = u32::MAX;

/// Compact fact request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ReqC {
    key: u32,
    pos: u32,
    term: u32,
    reason: RequestReason,
    image: bool,
}

#[derive(Clone, Debug)]
struct OpenInfo {
    status: OpenStatus,
    rule: Option<u32>,
    detail: String,
}

/// Per-key dispatch memo for the incremental closure.
#[derive(Clone, Debug)]
pub struct DispatchCache {
    tables: Vec<(String, usize)>,
    priority_names: Vec<String>,
    table: KeyTable,
    /// stopping (table, record) per key, or None
    stop: Vec<Option<(u32, u32)>>,
}

impl DispatchCache {
    pub fn len(&self) -> usize {
        self.table.len()
    }
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

/// The result of a closure run.
#[derive(Clone, Debug)]
pub struct Closure {
    tier: Tier,
    codec: KeyCodec,
    table: KeyTable,
    n_start: usize,
    start_mask: Vec<bool>,
    terminals: BTreeMap<u32, (TerminalKind, Option<u32>)>,
    inputs: Vec<InputLabel>,
    res: Vec<u32>,
    stop: Vec<u32>,
    open: BTreeMap<u32, OpenInfo>,
    credits: BTreeMap<u32, Credit>,
    rounds: u32,
    truncated: bool,
    edges: u64,
    terms: TermStats,
    requires_zero: u64,
    undecided: Undecided,
    violations: u64,
    pos_nonzero: u64,
    unrepresentable: u64,
    descent_witnesses: Vec<(u32, u32, Witness)>,
    failure_witnesses: Vec<(u32, u32, Witness)>,
    requests: Vec<ReqC>,
    hand_down_terms: BTreeMap<u64, u64>,
    hand_down_keys: BTreeMap<u64, KeySet>,
    cycles: Option<u64>,
    chain: Option<Chain>,
    rule_names: Vec<RuleName>,
    fact_tables: Vec<String>,
    table_names: Vec<String>,
    entry_table: Vec<u32>,
    priority_names: Vec<String>,
    tables_id: Vec<(String, usize)>,
    fact_label: String,
    facts_exact: bool,
    order_digest: String,
    start_digest: String,
    lines: Vec<bool>,
    pub seconds: f64,
    pub memory_estimate_bytes: usize,
}

/// sha256 over the dispatch order: per table in priority order its name, record count and labels.
pub fn dispatch_order_digest(disp: &Dispatcher) -> String {
    let mut h = sha256::Sha256::new();
    h.update(b"rustred.campaign.dispatch-order.v1\n");
    for &t in disp.priority() {
        let tab = &disp.tables()[t];
        h.update(tab.name.as_bytes());
        h.update(b"\n");
        h.update(&(tab.len() as u64).to_le_bytes());
        for l in &tab.labels {
            h.update(&l.to_le_bytes());
        }
    }
    format!("sha256:{}", h.finish_hex())
}

fn face_name(lines: &[bool], key: &[i32]) -> Option<String> {
    let mut d = vec![];
    let mut n = vec![];
    for (s, (&v, &l)) in key.iter().zip(lines).enumerate() {
        if l {
            if v < 1 {
                return None;
            }
            if v >= 2 {
                d.push(s.to_string());
            }
        } else {
            if v > 0 {
                return None;
            }
            if v <= -1 {
                n.push(s.to_string());
            }
        }
    }
    Some(format!("{{{}}}x{{{}}}", d.join(","), n.join(",")))
}

struct Ctx<'a> {
    disp: &'a Dispatcher,
    fast: &'a FastDispatch<'a>,
    bodies: &'a RuleBodies,
    cmp: &'a NodeComparator,
    cfg: &'a ClosureConfig,
    facts: &'a dyn FactSource,
    sectors: Sectors,
    codec: KeyCodec,
    arity: usize,
    rule_names: Vec<RuleName>,
    fact_tables: Vec<String>,
}

impl Ctx<'_> {
    fn rid(&self, pos: u32) -> RuleId<'_> {
        let r = &self.rule_names[pos as usize];
        RuleId {
            table: &self.fact_tables[pos as usize],
            record: r.record,
            label: r.label,
        }
    }
}

/// Output of expanding one chunk of keys.
#[derive(Default)]
struct ExpandOut {
    new_codes: Vec<u64>,
    edges: u64,
    terms: TermStats,
    requires_zero: u64,
    undecided: Undecided,
    violations: u64,
    pos_nonzero: u64,
    unrepresentable: u64,
    descent_w: Vec<(u32, u32, Witness)>,
    failure_w: Vec<(u32, u32, Witness)>,
    requests: Vec<ReqC>,
    hd_terms: BTreeMap<u64, u64>,
    hd_codes: BTreeMap<u64, Vec<u64>>,
}

fn push_capped(v: &mut Vec<(u32, u32, Witness)>, w: (u32, u32, Witness), cap: usize) {
    if v.len() < cap {
        v.push(w);
    } else if let Some(max) = v.iter().map(|x| (x.0, x.1)).max()
        && (w.0, w.1) < max
    {
        let i = v
            .iter()
            .position(|x| (x.0, x.1) == max)
            .expect("max present");
        v[i] = w;
    }
}

fn merge_capped(dst: &mut Vec<(u32, u32, Witness)>, src: Vec<(u32, u32, Witness)>, cap: usize) {
    dst.extend(src);
    dst.sort_by_key(|a| (a.0, a.1));
    dst.truncate(cap);
}

/// What the expansion of one key produces (shared by the BFS and the depth-first chain pass).
enum Sink<'s> {
    Bfs {
        out: &'s mut ExpandOut,
        table: &'s KeyTable,
        key_idx: u32,
    },
    Succ {
        table: &'s KeyTable,
        succ: &'s mut Vec<u32>,
    },
}

/// Expand one key: `applied` = the key the rule is instantiated at (the image for a credit), `key` = the key itself.
#[allow(clippy::too_many_arguments)]
fn expand_key(
    ctx: &Ctx,
    pos: u32,
    applied: &[i32],
    key: &[i32],
    image: bool,
    buf: &mut Vec<i32>,
    cm: &mut Vec<u64>,
    codes: &mut Vec<u64>,
    sink: &mut Sink,
) -> Result<(), ClosureError> {
    let rule = ctx.fast.rule(pos);
    buf.clear();
    cm.clear();
    codes.clear();
    let n = ctx.arity;
    let w = ctx.codec.words();
    let Some(nt) = ctx.bodies.instantiate(ctx.disp, rule, applied, buf, cm)? else {
        return Ok(());
    };
    let tier_c = ctx.cfg.tier == Tier::C;
    let cap = ctx.cfg.witness_cap;
    let mut code = [0u64; keyset::MAX_WORDS];
    for j in 0..nt {
        let t = &buf[j * n..(j + 1) * n];
        let class = ctx.sectors.classify_masked(t, cm[j]);
        let fact = if tier_c {
            ctx.facts.coefficient(&ctx.rid(pos), applied, j)
        } else {
            CoefFact::Unknown
        };
        if let Sink::Bfs { out, key_idx, .. } = sink {
            out.terms.total += 1;
            if fact == CoefFact::Zero {
                out.terms.dropped_zero += 1;
                continue;
            }
            match class {
                TermClass::PositiveNumerator {
                    symbolic,
                    subsector,
                    ..
                } => {
                    if subsector {
                        out.terms.positive_numerator_sub += 1;
                    } else {
                        out.terms.positive_numerator_top += 1;
                    }
                    if symbolic {
                        out.terms.positive_numerator_symbolic += 1;
                    } else {
                        out.terms.positive_numerator_constant += 1;
                    }
                    if !tier_c {
                        out.requires_zero += 1;
                    } else if fact == CoefFact::NonZero {
                        out.pos_nonzero += 1;
                        push_capped(
                            &mut out.failure_w,
                            (
                                *key_idx,
                                j as u32,
                                witness(ctx, pos, key, j, t, "positive_numerator_nonzero"),
                            ),
                            cap,
                        );
                    } else {
                        out.undecided.coef_unknown_positive_numerator += 1;
                        out.requests.push(ReqC {
                            key: *key_idx,
                            pos,
                            term: j as u32,
                            reason: RequestReason::PositiveNumerator,
                            image,
                        });
                    }
                    continue;
                }
                TermClass::ZeroSector { .. } => {
                    out.terms.zero_sector += 1;
                    continue;
                }
                TermClass::Subsector { dropped } => {
                    out.terms.subsector += 1;
                    match ctx.cfg.hand_downs {
                        HandDowns::Off => {}
                        HandDowns::Counts => *out.hd_terms.entry(dropped).or_default() += 1,
                        HandDowns::Keys => {
                            *out.hd_terms.entry(dropped).or_default() += 1;
                            if ctx.codec.encode(t, &mut code) {
                                out.hd_codes
                                    .entry(dropped)
                                    .or_default()
                                    .extend_from_slice(&code[..w]);
                            } else {
                                out.unrepresentable += 1;
                                push_capped(
                                    &mut out.failure_w,
                                    (
                                        *key_idx,
                                        j as u32,
                                        witness(ctx, pos, key, j, t, "unrepresentable_hand_down"),
                                    ),
                                    cap,
                                );
                            }
                        }
                    }
                    continue;
                }
                TermClass::Top => {
                    out.terms.top += 1;
                    if !ctx.cmp.lower(t, key) {
                        if !tier_c || fact == CoefFact::NonZero {
                            out.violations += 1;
                            push_capped(
                                &mut out.descent_w,
                                (
                                    *key_idx,
                                    j as u32,
                                    witness(ctx, pos, key, j, t, "descent_violation"),
                                ),
                                cap,
                            );
                        } else {
                            out.undecided.coef_unknown_not_lower += 1;
                            out.requests.push(ReqC {
                                key: *key_idx,
                                pos,
                                term: j as u32,
                                reason: RequestReason::NotLower,
                                image,
                            });
                        }
                    }
                    if ctx.codec.encode(t, &mut code) {
                        codes.extend_from_slice(&code[..w]);
                    } else {
                        out.unrepresentable += 1;
                        push_capped(
                            &mut out.failure_w,
                            (
                                *key_idx,
                                j as u32,
                                witness(ctx, pos, key, j, t, "unrepresentable_key"),
                            ),
                            cap,
                        );
                    }
                }
            }
        } else {
            if fact == CoefFact::Zero || class != TermClass::Top {
                continue;
            }
            if ctx.codec.encode(t, &mut code) {
                codes.extend_from_slice(&code[..w]);
            }
        }
    }
    keyset::sort_dedup(w, codes);
    match sink {
        Sink::Bfs { out, table, .. } => {
            out.edges += (codes.len() / w) as u64;
            for c in codes.chunks_exact(w) {
                if table.get(c).is_none() {
                    out.new_codes.extend_from_slice(c);
                }
            }
        }
        Sink::Succ { table, succ } => {
            for c in codes.chunks_exact(w) {
                if let Some(i) = table.get(c) {
                    succ.push(i);
                }
            }
        }
    }
    Ok(())
}

fn witness(ctx: &Ctx, pos: u32, key: &[i32], term: usize, rhs: &[i32], reason: &str) -> Witness {
    Witness {
        key: key.to_vec(),
        rule: ctx.rule_names[pos as usize].clone(),
        term,
        rhs: rhs.to_vec(),
        reason: reason.into(),
    }
}

/// Split `lo..hi` into at most `threads` contiguous chunks.
fn chunks(lo: usize, hi: usize, threads: usize) -> Vec<(usize, usize)> {
    let n = hi - lo;
    let t = threads.max(1).min(n.max(1));
    let c = n.div_ceil(t).max(1);
    (0..t)
        .map(|i| (lo + i * c, (lo + (i + 1) * c).min(hi)))
        .filter(|(a, b)| a < b)
        .collect()
}

/// Dispatch result of one key (internal).
enum KeyRes {
    Terminal(TerminalKind, Option<u32>),
    Pos(u32, Option<RegFact>),
    Credit(Box<Credit>),
    Open(OpenInfo),
}

/// Forward closure of `start` (see the module documentation).
pub fn closure(
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    start: &KeySet,
    cfg: &ClosureConfig,
    facts: &dyn FactSource,
) -> Result<Closure, ClosureError> {
    run(disp, bodies, cmp, start, cfg, facts, None, None)
}

/// Keys-only RECHECK / extension (R41 P-2): continue a previous closure with additional start keys (e.g. the proved
/// envelope replacing a measured one). Only keys not seen before are dispatched and expanded; everything already
/// computed is kept. Requires the same dispatch order, tier and codec bits (else `NotIncremental`). The result
/// equals a from-scratch closure of (previous start set + `extra`) (tested). The labels in `cfg.inputs` are added
/// to the previous ones.
pub fn closure_recheck(
    prev: &Closure,
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    extra: &KeySet,
    cfg: &ClosureConfig,
    facts: &dyn FactSource,
) -> Result<Closure, ClosureError> {
    if prev.order_digest != dispatch_order_digest(disp) {
        return Err(ClosureError::NotIncremental(
            "dispatch order differs from the previous closure".into(),
        ));
    }
    if prev.tier != cfg.tier || prev.codec.bits() != cfg.bits {
        return Err(ClosureError::NotIncremental(
            "tier or codec bits differ".into(),
        ));
    }
    if prev.truncated {
        return Err(ClosureError::NotIncremental(
            "previous closure was truncated".into(),
        ));
    }
    run(disp, bodies, cmp, extra, cfg, facts, None, Some(prev))
}

/// Incremental closure: as [`closure`], reusing the dispatch results of `prev` where the invariant of the interface
/// (section 6) guarantees they are unchanged.
pub fn closure_incremental(
    prev: &DispatchCache,
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    start: &KeySet,
    cfg: &ClosureConfig,
    facts: &dyn FactSource,
) -> Result<Closure, ClosureError> {
    let tabs = disp.tables();
    if prev.tables.len() > tabs.len() {
        return Err(ClosureError::NotIncremental(
            "fewer tables than before".into(),
        ));
    }
    for (i, (name, len)) in prev.tables.iter().enumerate() {
        if tabs[i].name != *name || tabs[i].len() != *len {
            return Err(ClosureError::NotIncremental(format!(
                "table {i} ({name}) differs"
            )));
        }
    }
    let new_order: Vec<String> = disp
        .priority()
        .iter()
        .filter(|&&t| t < prev.tables.len())
        .map(|&t| tabs[t].name.clone())
        .collect();
    if new_order != prev.priority_names {
        return Err(ClosureError::NotIncremental(
            "relative order of the old tables changed".into(),
        ));
    }
    run(disp, bodies, cmp, start, cfg, facts, Some(prev), None)
}

#[allow(clippy::too_many_arguments)]
fn run(
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    start: &KeySet,
    cfg: &ClosureConfig,
    facts: &dyn FactSource,
    cache: Option<&DispatchCache>,
    resume: Option<&Closure>,
) -> Result<Closure, ClosureError> {
    let t0 = Instant::now();
    let arity = disp.arity();
    if start.codec().arity() != arity {
        return Err(ClosureError::KeyArity {
            expected: arity,
            got: start.codec().arity(),
        });
    }
    if cmp.arity() != arity {
        return Err(ClosureError::KeyArity {
            expected: arity,
            got: cmp.arity(),
        });
    }
    let lines: Vec<bool> = match (disp.chart(), &cfg.lines) {
        (_, Some(l)) => l.clone(),
        (Some(c), None) => c.to_vec(),
        (None, None) => return Err(ClosureError::NoLines),
    };
    if lines.len() != arity {
        return Err(ClosureError::KeyArity {
            expected: arity,
            got: lines.len(),
        });
    }
    let codec = KeyCodec::new(arity, cfg.bits)?;
    let fast = FastDispatch::new(disp);
    let n_entries = fast.n_entries();
    let rule_names: Vec<RuleName> = (0..n_entries as u32)
        .map(|p| {
            let r = fast.rule(p);
            RuleName {
                table: disp.tables()[r.table].name.clone(),
                record: r.record,
                label: r.label,
            }
        })
        .collect();
    let entry_table: Vec<u32> = (0..n_entries as u32)
        .map(|p| fast.rule(p).table as u32)
        .collect();
    let ctx = Ctx {
        disp,
        fast: &fast,
        bodies,
        cmp,
        cfg,
        facts,
        sectors: Sectors {
            lines: lines.clone(),
            zero_sectors: cfg.zero_sectors.clone(),
        },
        codec,
        arity,
        fact_tables: rule_names
            .iter()
            .map(|r| {
                cfg.fact_tables.get(&r.table).cloned().unwrap_or_else(|| {
                    disp.tables()
                        .iter()
                        .find(|t| t.name == r.table)
                        .map(|t| t.provenance.source.clone())
                        .unwrap_or_default()
                })
            })
            .collect(),
        rule_names,
    };
    // start keys (canonical order)
    let start = if *start.codec() == codec {
        start.clone()
    } else {
        start.recode(codec)?
    };
    let recode_opt = |k: &Option<KeySet>| -> Result<Option<KeySet>, ClosureError> {
        Ok(match k {
            None => None,
            Some(k) if *k.codec() == codec => Some(k.clone()),
            Some(k) => Some(k.recode(codec)?),
        })
    };
    let (term_pref, term_other) = match &cfg.terminals {
        None => (None, None),
        Some(t) => (recode_opt(&t.preferred)?, recode_opt(&t.other)?),
    };
    // cache translation (incremental)
    let old_n_tables = cache.map(|c| c.tables.len()).unwrap_or(0);
    let is_new =
        |p: u32| -> bool { cache.is_some() && entry_table[p as usize] as usize >= old_n_tables };

    let w = codec.words();
    let mut table;
    let mut res: Vec<u32>;
    let mut stop: Vec<u32>;
    let mut open: BTreeMap<u32, OpenInfo>;
    let mut credits: BTreeMap<u32, Credit>;
    let mut terminals: BTreeMap<u32, (TerminalKind, Option<u32>)>;
    let mut acc = ExpandOut::default();
    let mut hd_keys: BTreeMap<u64, Vec<u64>>;
    let mut start_mask: Vec<bool>;
    let mut rounds = 0u32;
    let mut truncated = false;
    let mut inputs: Vec<InputLabel> = vec![];
    match resume {
        None => {
            table = KeyTable::new(codec, start.len().max(1) * 2);
            res = vec![];
            stop = vec![];
            open = BTreeMap::new();
            credits = BTreeMap::new();
            terminals = BTreeMap::new();
            hd_keys = BTreeMap::new();
            start_mask = vec![];
        }
        Some(p) => {
            table = p.table.clone();
            res = p.res.clone();
            stop = p.stop.clone();
            open = p.open.clone();
            credits = p.credits.clone();
            terminals = p.terminals.clone();
            hd_keys = p
                .hand_down_keys
                .iter()
                .map(|(&m, k)| (m, k.raw_codes().to_vec()))
                .collect();
            start_mask = p.start_mask.clone();
            rounds = p.rounds;
            inputs = p.inputs.clone();
            acc.edges = p.edges;
            acc.terms = p.terms.clone();
            acc.requires_zero = p.requires_zero;
            acc.undecided = p.undecided.clone();
            acc.violations = p.violations;
            acc.pos_nonzero = p.pos_nonzero;
            acc.unrepresentable = p.unrepresentable;
            acc.descent_w = p.descent_witnesses.clone();
            acc.failure_w = p.failure_witnesses.clone();
            acc.requests = p.requests.clone();
            acc.hd_terms = p.hand_down_terms.clone();
        }
    }
    inputs.extend(cfg.inputs.iter().cloned());
    inputs.sort();
    inputs.dedup();
    let mut lo = table.len();
    for i in 0..start.len() {
        let (ix, new) = table.insert(start.code(i));
        if new {
            res.push(PENDING);
            stop.push(NO_STOP);
            start_mask.push(true);
        } else {
            start_mask[ix as usize] = true;
        }
    }
    if resume.is_none() {
        lo = 0;
    }
    drop(start);
    let rounds_before = rounds;
    loop {
        let hi = table.len();
        if lo == hi {
            break;
        }
        if cfg.max_rounds.is_some_and(|m| rounds >= m) {
            truncated = true;
            for (i, r) in res.iter_mut().enumerate().take(hi).skip(lo) {
                *r = OPEN;
                open.insert(
                    i as u32,
                    OpenInfo {
                        status: OpenStatus::Truncated,
                        rule: None,
                        detail: "round limit".into(),
                    },
                );
            }
            break;
        }
        rounds += 1;
        // ---- dispatch
        let parts = chunks(lo, hi, cfg.threads);
        let results: Vec<Result<Vec<(KeyRes, u32)>, ClosureError>> = std::thread::scope(|s| {
            let hs: Vec<_> = parts
                .iter()
                .map(|&(a, b)| {
                    let (ctx, table, is_new) = (&ctx, &table, &is_new);
                    let (term_pref, term_other) = (&term_pref, &term_other);
                    s.spawn(move || -> Result<Vec<(KeyRes, u32)>, ClosureError> {
                        let mut buf = vec![];
                        let mut st = vec![];
                        let mut key = vec![0i32; arity];
                        let mut v = Vec::with_capacity(b - a);
                        for i in a..b {
                            codec.decode(table.code(i), &mut key);
                            let tk = if term_pref.as_ref().is_some_and(|t| t.contains(&key)) {
                                Some(TerminalKind::Preferred)
                            } else if term_other.as_ref().is_some_and(|t| t.contains(&key)) {
                                Some(TerminalKind::Other)
                            } else {
                                None
                            };
                            if let Some(kind) = tk {
                                let would = match ctx.fast.first_match_with(&key, &mut buf, &mut st)
                                {
                                    Dispatch::Covered(r) => Some(ctx.fast.position(r)),
                                    _ => None,
                                };
                                v.push((KeyRes::Terminal(kind, would), NO_STOP));
                                continue;
                            }
                            let d = match cache {
                                Some(c) => {
                                    let old_stop = match c.table.get(table.code(i)) {
                                        None => 0, // never seen: evaluate everything
                                        Some(ci) => match c.stop[ci as usize] {
                                            None => u32::MAX,
                                            Some((t, r)) => ctx.fast.position(RuleRef {
                                                table: t as usize,
                                                record: r as usize,
                                                label: 0,
                                            }),
                                        },
                                    };
                                    ctx.fast
                                        .first_match_cached(
                                            &key, old_stop, is_new, &mut buf, &mut st,
                                        )
                                        .0
                                }
                                None => ctx.fast.first_match_with(&key, &mut buf, &mut st),
                            };
                            v.push(dispatch_one(ctx, &key, d)?);
                        }
                        Ok(v)
                    })
                })
                .collect();
            hs.into_iter()
                .map(|h| h.join().expect("dispatch worker"))
                .collect()
        });
        let mut i = lo;
        for part in results {
            for (kr, st) in part? {
                stop[i] = st;
                match kr {
                    KeyRes::Pos(p, reg) => {
                        res[i] = p;
                        if reg == Some(RegFact::Unknown) {
                            acc.undecided.regularity_unknown += 1;
                            acc.requests.push(ReqC {
                                key: i as u32,
                                pos: p,
                                term: u32::MAX,
                                reason: RequestReason::Regularity,
                                image: false,
                            });
                        }
                    }
                    KeyRes::Credit(c) => {
                        res[i] = CREDITED;
                        credits.insert(i as u32, *c);
                    }
                    KeyRes::Open(o) => {
                        res[i] = OPEN;
                        open.insert(i as u32, o);
                    }
                    KeyRes::Terminal(kind, would) => {
                        res[i] = TERMINAL;
                        terminals.insert(i as u32, (kind, would));
                    }
                }
                i += 1;
            }
        }
        // ---- expand
        let outs: Vec<Result<ExpandOut, ClosureError>> = std::thread::scope(|s| {
            let hs: Vec<_> = parts
                .iter()
                .map(|&(a, b)| {
                    let (ctx, table, res, credits) = (&ctx, &table, &res, &credits);
                    s.spawn(move || -> Result<ExpandOut, ClosureError> {
                        let mut out = ExpandOut::default();
                        let (mut buf, mut cm, mut codes) = (vec![], vec![], vec![]);
                        let mut key = vec![0i32; arity];
                        for i in a..b {
                            let r = res[i];
                            let (pos, image) = match r {
                                OPEN | PENDING | TERMINAL => continue,
                                CREDITED => (credits[&(i as u32)].position, true),
                                p => (p, false),
                            };
                            codec.decode(table.code(i), &mut key);
                            let applied: Vec<i32> = if image {
                                credits[&(i as u32)].image.clone()
                            } else {
                                key.clone()
                            };
                            let mut sink = Sink::Bfs {
                                out: &mut out,
                                table,
                                key_idx: i as u32,
                            };
                            expand_key(
                                ctx, pos, &applied, &key, image, &mut buf, &mut cm, &mut codes,
                                &mut sink,
                            )?;
                        }
                        keyset::sort_dedup(w, &mut out.new_codes);
                        for v in out.hd_codes.values_mut() {
                            keyset::sort_dedup(w, v);
                        }
                        Ok(out)
                    })
                })
                .collect();
            hs.into_iter()
                .map(|h| h.join().expect("expand worker"))
                .collect()
        });
        let mut new_codes = vec![];
        for o in outs {
            let o = o?;
            new_codes.extend_from_slice(&o.new_codes);
            acc.edges += o.edges;
            acc.terms.add(&o.terms);
            acc.requires_zero += o.requires_zero;
            acc.undecided.coef_unknown_positive_numerator +=
                o.undecided.coef_unknown_positive_numerator;
            acc.undecided.coef_unknown_not_lower += o.undecided.coef_unknown_not_lower;
            acc.violations += o.violations;
            acc.pos_nonzero += o.pos_nonzero;
            acc.unrepresentable += o.unrepresentable;
            merge_capped(&mut acc.descent_w, o.descent_w, cfg.witness_cap);
            merge_capped(&mut acc.failure_w, o.failure_w, cfg.witness_cap);
            acc.requests.extend(o.requests);
            for (k, v) in o.hd_terms {
                *acc.hd_terms.entry(k).or_default() += v;
            }
            for (k, v) in o.hd_codes {
                hd_keys.entry(k).or_default().extend(v);
            }
        }
        for v in hd_keys.values_mut() {
            keyset::sort_dedup(w, v);
        }
        keyset::sort_dedup(w, &mut new_codes);
        for c in new_codes.chunks_exact(w) {
            table.insert(c);
        }
        res.resize(table.len(), PENDING);
        stop.resize(table.len(), NO_STOP);
        start_mask.resize(table.len(), false);
        lo = hi;
    }
    let _ = rounds_before;
    let n_start = start_mask.iter().filter(|&&b| b).count();
    let start_digest = {
        let mut d = vec![];
        for (i, &m) in start_mask.iter().enumerate() {
            if m {
                d.extend_from_slice(table.code(i));
            }
        }
        KeySet::from_codes(codec, d).digest()
    };
    let mut cl = Closure {
        tier: cfg.tier,
        codec,
        n_start,
        start_mask,
        terminals,
        inputs,
        open,
        credits,
        rounds,
        truncated,
        edges: acc.edges,
        terms: acc.terms,
        requires_zero: acc.requires_zero,
        undecided: acc.undecided,
        violations: acc.violations,
        pos_nonzero: acc.pos_nonzero,
        unrepresentable: acc.unrepresentable,
        descent_witnesses: acc.descent_w,
        failure_witnesses: acc.failure_w,
        requests: acc.requests,
        hand_down_terms: acc.hd_terms,
        hand_down_keys: hd_keys
            .into_iter()
            .map(|(k, v)| (k, KeySet::from_codes(codec, v)))
            .collect(),
        cycles: None,
        chain: None,
        rule_names: ctx.rule_names.clone(),
        fact_tables: ctx.fact_tables.clone(),
        table_names: disp.tables().iter().map(|t| t.name.clone()).collect(),
        entry_table,
        priority_names: disp
            .priority()
            .iter()
            .map(|&t| disp.tables()[t].name.clone())
            .collect(),
        tables_id: disp
            .tables()
            .iter()
            .map(|t| (t.name.clone(), t.len()))
            .collect(),
        fact_label: facts.label(),
        facts_exact: facts.exact(),
        order_digest: dispatch_order_digest(disp),
        start_digest,
        lines: lines.clone(),
        seconds: 0.0,
        memory_estimate_bytes: 0,
        table,
        res,
        stop,
    };
    if cfg.longest_chain && !cl.truncated {
        chain_pass(&ctx, &mut cl)?;
    }
    cl.memory_estimate_bytes = cl.table.memory_bytes()
        + cl.res.capacity() * 4
        + cl.stop.capacity() * 4
        + bodies.memory_bytes()
        + fast.memory_bytes()
        + cl.hand_down_keys
            .values()
            .map(|k| k.memory_bytes())
            .sum::<usize>()
        + cl.requests.capacity() * std::mem::size_of::<ReqC>();
    cl.seconds = t0.elapsed().as_secs_f64();
    Ok(cl)
}

/// Dispatch one key: covered (with Tier C regularity), credited, or open. Returns the stopping position too.
fn dispatch_one(ctx: &Ctx, key: &[i32], d: Dispatch) -> Result<(KeyRes, u32), ClosureError> {
    Ok(match d {
        Dispatch::Covered(r) => {
            let p = ctx.fast.position(r);
            if !ctx.bodies.has(r.table, r.record) {
                return Ok((
                    KeyRes::Open(OpenInfo {
                        status: OpenStatus::NoBody,
                        rule: Some(p),
                        detail: String::new(),
                    }),
                    p,
                ));
            }
            if ctx.cfg.tier == Tier::C {
                match ctx.facts.regularity(&ctx.rid(p), key) {
                    RegFact::Pole => {
                        return Ok((
                            KeyRes::Open(OpenInfo {
                                status: OpenStatus::Irregular,
                                rule: Some(p),
                                detail: "pole".into(),
                            }),
                            p,
                        ));
                    }
                    RegFact::Unknown if ctx.cfg.require_regularity => {
                        (KeyRes::Pos(p, Some(RegFact::Unknown)), p)
                    }
                    f => (KeyRes::Pos(p, Some(f)), p),
                }
            } else {
                (KeyRes::Pos(p, None), p)
            }
        }
        Dispatch::Uncovered => {
            if let Some(pol) = &ctx.cfg.credits {
                match crate::symmetry::serve_with(key, pol, ctx)? {
                    Ok(c) => return Ok((KeyRes::Credit(Box::new(c)), NO_STOP)),
                    Err(refusals) => {
                        let detail = format!("credit refused by {} element(s)", refusals.len());
                        return Ok((
                            KeyRes::Open(OpenInfo {
                                status: OpenStatus::Uncovered,
                                rule: None,
                                detail,
                            }),
                            NO_STOP,
                        ));
                    }
                }
            }
            (
                KeyRes::Open(OpenInfo {
                    status: OpenStatus::Uncovered,
                    rule: None,
                    detail: String::new(),
                }),
                NO_STOP,
            )
        }
        Dispatch::Blocked { at, reason } => {
            let p = ctx.fast.position(at);
            (
                KeyRes::Open(OpenInfo {
                    status: OpenStatus::Blocked,
                    rule: Some(p),
                    detail: format!("{reason:?}"),
                }),
                p,
            )
        }
        Dispatch::Undecided { at, verdict } => {
            let p = ctx.fast.position(at);
            (
                KeyRes::Open(OpenInfo {
                    status: OpenStatus::Undecided,
                    rule: Some(p),
                    detail: format!("{verdict:?}"),
                }),
                p,
            )
        }
        Dispatch::OutsideChart { slot } => (
            KeyRes::Open(OpenInfo {
                status: OpenStatus::OutsideChart,
                rule: None,
                detail: format!("slot {slot}"),
            }),
            NO_STOP,
        ),
    })
}

/// Memoised depth-first search over the actual edges: cycle check and longest chain (edges). Successors are
/// recomputed (no stored edges). Deterministic: roots in key-index order, ties to the smallest successor index.
fn chain_pass(ctx: &Ctx, cl: &mut Closure) -> Result<(), ClosureError> {
    let n = cl.table.len();
    let mut state = vec![0u8; n]; // 0 new, 1 on stack, 2 done
    let mut h = vec![0u32; n];
    let mut next = vec![u32::MAX; n];
    let mut cycles = 0u64;
    let (mut buf, mut cm, mut codes) = (vec![], vec![], vec![]);
    let mut key = vec![0i32; ctx.arity];
    // stack frames: (node, successor list, cursor)
    let mut stack: Vec<(u32, Vec<u32>, usize)> = vec![];
    let succ_of = |i: u32,
                   buf: &mut Vec<i32>,
                   cm: &mut Vec<u64>,
                   codes: &mut Vec<u64>,
                   key: &mut Vec<i32>|
     -> Result<Vec<u32>, ClosureError> {
        let mut succ = vec![];
        let r = cl.res[i as usize];
        let (pos, image) = match r {
            OPEN | PENDING | TERMINAL => return Ok(succ),
            CREDITED => (cl.credits[&i].position, true),
            p => (p, false),
        };
        ctx.codec.decode(cl.table.code(i as usize), key);
        let applied = if image {
            cl.credits[&i].image.clone()
        } else {
            key.clone()
        };
        let mut sink = Sink::Succ {
            table: &cl.table,
            succ: &mut succ,
        };
        expand_key(ctx, pos, &applied, key, image, buf, cm, codes, &mut sink)?;
        Ok(succ)
    };
    for root in 0..n as u32 {
        if state[root as usize] != 0 {
            continue;
        }
        state[root as usize] = 1;
        let s = succ_of(root, &mut buf, &mut cm, &mut codes, &mut key)?;
        stack.push((root, s, 0));
        while let Some(top) = stack.last_mut() {
            let (v, ref succ, ref mut cur) = *top;
            if *cur < succ.len() {
                let u = succ[*cur];
                *cur += 1;
                match state[u as usize] {
                    0 => {
                        state[u as usize] = 1;
                        let s = succ_of(u, &mut buf, &mut cm, &mut codes, &mut key)?;
                        stack.push((u, s, 0));
                    }
                    1 => cycles += 1,
                    _ => {}
                }
            } else {
                let mut best = 0u32;
                let mut bn = u32::MAX;
                for &u in succ {
                    if state[u as usize] == 2
                        && (h[u as usize] + 1 > best || (h[u as usize] + 1 == best && u < bn))
                    {
                        best = h[u as usize] + 1;
                        bn = u;
                    }
                }
                h[v as usize] = best;
                next[v as usize] = bn;
                state[v as usize] = 2;
                stack.pop();
            }
        }
    }
    cl.cycles = Some(cycles);
    if n > 0 {
        let mut top = 0usize;
        for i in 0..n {
            if h[i] > h[top] {
                top = i;
            }
        }
        let mut sha = sha256::Sha256::new();
        let mut c = top;
        let mut last;
        let mut guard = 0usize;
        loop {
            for v in cl.table.key(c) {
                sha.update(&v.to_le_bytes());
            }
            last = c;
            if next[c] == u32::MAX || guard > n {
                break;
            }
            c = next[c] as usize;
            guard += 1;
        }
        cl.chain = Some(Chain {
            edges: h[top] as u64,
            from: cl.table.key(top),
            to: cl.table.key(last),
            longest_from_start_key: (0..n)
                .filter(|&i| cl.start_mask[i])
                .map(|i| h[i] as u64)
                .max()
                .unwrap_or(0),
            path_digest: format!("sha256:{}", sha.finish_hex()),
        });
    }
    Ok(())
}

impl Closure {
    pub fn tier(&self) -> Tier {
        self.tier
    }
    pub fn codec(&self) -> &KeyCodec {
        &self.codec
    }
    pub fn len(&self) -> usize {
        self.table.len()
    }
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
    pub fn n_start(&self) -> usize {
        self.n_start
    }
    /// All reached keys (canonical set).
    pub fn keys(&self) -> KeySet {
        self.table.to_keyset()
    }
    /// Reached keys outside the start set.
    pub fn outside_keys(&self) -> KeySet {
        let mut d = vec![];
        for i in 0..self.table.len() {
            if !self.start_mask[i] {
                d.extend_from_slice(self.table.code(i));
            }
        }
        KeySet::from_codes(self.codec, d)
    }
    /// Key number `i` in reach order (start keys first).
    pub fn key(&self, i: usize) -> Vec<i32> {
        self.table.key(i)
    }
    pub fn index_of(&self, key: &[i32]) -> Option<usize> {
        self.table.get_key(key).map(|i| i as usize)
    }
    /// The winning rule of key `i` (covered keys), or the credit's rule.
    pub fn winner(&self, i: usize) -> Option<RuleName> {
        match self.res[i] {
            OPEN | PENDING | TERMINAL => None,
            CREDITED => Some(self.rule_names[self.credits[&(i as u32)].position as usize].clone()),
            p => Some(self.rule_names[p as usize].clone()),
        }
    }
    pub fn is_open(&self, i: usize) -> bool {
        self.res[i] == OPEN
    }
    pub fn is_credited(&self, i: usize) -> bool {
        self.res[i] == CREDITED
    }
    /// The open keys (the defect), in reach order.
    pub fn open_keys(&self) -> Vec<OpenKey> {
        self.open
            .iter()
            .map(|(&i, o)| OpenKey {
                key: self.table.key(i as usize),
                in_start: self.start_mask[i as usize],
                status: o.status,
                rule: o.rule.map(|p| self.rule_names[p as usize].clone()),
                detail: o.detail.clone(),
            })
            .collect()
    }
    /// Open keys as a key set.
    pub fn open_keyset(&self) -> KeySet {
        let mut d = vec![];
        for &i in self.open.keys() {
            d.extend_from_slice(self.table.code(i as usize));
        }
        KeySet::from_codes(self.codec, d)
    }
    pub fn credits(&self) -> Vec<CreditRecord> {
        self.credits
            .iter()
            .map(|(&i, c)| CreditRecord {
                key: self.table.key(i as usize),
                in_start: self.start_mask[i as usize],
                element: c.element,
                image: c.image.clone(),
                rule: self.rule_names[c.position as usize].clone(),
                descent_checked: true,
            })
            .collect()
    }
    /// Whether key `i` belongs to the start set (all start sets of a rechecked closure).
    pub fn in_start(&self, i: usize) -> bool {
        self.start_mask[i]
    }
    /// Declared terminals reached (reach order).
    pub fn terminal_records(&self) -> Vec<TerminalRecord> {
        self.terminals
            .iter()
            .map(|(&i, &(kind, would))| TerminalRecord {
                key: self.table.key(i as usize),
                in_start: self.start_mask[i as usize],
                kind,
                would_dispatch: would.map(|p| self.rule_names[p as usize].clone()),
            })
            .collect()
    }
    pub fn is_terminal(&self, i: usize) -> bool {
        self.res[i] == TERMINAL
    }
    pub fn hand_down_sets(&self) -> &BTreeMap<u64, KeySet> {
        &self.hand_down_keys
    }
    /// Fact requests (sorted, de-duplicated).
    pub fn fact_requests(&self) -> Vec<FactRequest> {
        let mut v: Vec<FactRequest> = self
            .requests
            .iter()
            .map(|r| {
                let rn = &self.rule_names[r.pos as usize];
                let key = if r.image {
                    self.credits[&r.key].image.clone()
                } else {
                    self.table.key(r.key as usize)
                };
                FactRequest {
                    schema: facts::FACT_REQUEST_SCHEMA.into(),
                    table: self.fact_tables[r.pos as usize].clone(),
                    record: rn.record,
                    label: rn.label,
                    key,
                    kind: if r.reason == RequestReason::Regularity {
                        FactKind::Regularity
                    } else {
                        FactKind::Coefficient
                    },
                    term: (r.term != u32::MAX).then_some(r.term as usize),
                    rhs: None,
                    reason: r.reason,
                }
            })
            .collect();
        v.sort();
        v.dedup();
        v
    }
    /// Per dispatch position: (fires on all keys, fires on start keys).
    fn fires(&self) -> Vec<(u64, u64)> {
        let mut f = vec![(0u64, 0u64); self.rule_names.len()];
        for (i, &r) in self.res.iter().enumerate() {
            let p = match r {
                OPEN | PENDING | TERMINAL => continue,
                CREDITED => self.credits[&(i as u32)].position,
                p => p,
            };
            f[p as usize].0 += 1;
            if self.start_mask[i] {
                f[p as usize].1 += 1;
            }
        }
        f
    }
    /// Used rules with their firing counts (dispatch order).
    pub fn used_rules(&self) -> Vec<RuleUse> {
        self.fires()
            .into_iter()
            .enumerate()
            .filter(|(_, f)| f.0 > 0)
            .map(|(p, f)| RuleUse {
                rule: self.rule_names[p].clone(),
                fires: f.0,
                fires_in_start: f.1,
            })
            .collect()
    }
    pub fn descent_witnesses(&self) -> Vec<Witness> {
        self.descent_witnesses.iter().map(|x| x.2.clone()).collect()
    }
    pub fn failure_witnesses(&self) -> Vec<Witness> {
        self.failure_witnesses.iter().map(|x| x.2.clone()).collect()
    }
    /// The dispatch memo for [`closure_incremental`].
    pub fn dispatch_cache(&self) -> DispatchCache {
        DispatchCache {
            tables: self.tables_id.clone(),
            priority_names: self.priority_names.clone(),
            table: self.table.clone(),
            stop: self
                .stop
                .iter()
                .map(|&p| {
                    (p != NO_STOP).then(|| {
                        let r = &self.rule_names[p as usize];
                        (self.entry_table[p as usize], r.record as u32)
                    })
                })
                .collect(),
        }
    }

    fn verdict(&self) -> Verdict {
        let failed = self.violations > 0
            || self.pos_nonzero > 0
            || self.unrepresentable > 0
            || self.cycles.is_some_and(|c| c > 0);
        if failed {
            Verdict::Failed
        } else if !self.open.is_empty() {
            Verdict::Open
        } else if self.tier == Tier::K {
            Verdict::SupersetClean
        } else if self.undecided.total() > 0 || self.truncated {
            Verdict::Undecided
        } else if self.facts_exact && !self.inputs.iter().any(|x| x.provisional_input()) {
            Verdict::Closed
        } else {
            Verdict::ClosedProvisional
        }
    }

    /// The numbers.
    pub fn summary(&self) -> ClosureSummary {
        let lines: Vec<bool> = {
            // the line mask is recoverable from any face computation; keep it from the sectors of the run
            self.lines_mask()
        };
        let mut by_status: BTreeMap<String, u64> = BTreeMap::new();
        let mut by_face: BTreeMap<String, [u64; 2]> = BTreeMap::new();
        let mut open_in = 0u64;
        for (&i, o) in &self.open {
            let in_start = self.start_mask[i as usize];
            if in_start {
                open_in += 1;
            }
            *by_status
                .entry(format!("{:?}", o.status).to_lowercase())
                .or_default() += 1;
            if let Some(f) = face_name(&lines, &self.table.key(i as usize)) {
                by_face.entry(f).or_default()[if in_start { 0 } else { 1 }] += 1;
            }
        }
        let fires = self.fires();
        let mut wins: BTreeMap<String, [u64; 2]> = BTreeMap::new();
        for name in &self.table_names {
            wins.insert(name.clone(), [0, 0]);
        }
        for (p, f) in fires.iter().enumerate() {
            let e = wins
                .get_mut(&self.table_names[self.entry_table[p] as usize])
                .expect("table");
            e[0] += f.0;
            e[1] += f.1;
        }
        let hand_downs = self
            .hand_down_terms
            .iter()
            .map(|(&m, &t)| {
                (
                    m,
                    [
                        t,
                        self.hand_down_keys
                            .get(&m)
                            .map(|k| k.len() as u64)
                            .unwrap_or(0),
                    ],
                )
            })
            .collect();
        ClosureSummary {
            tier: self.tier,
            superset: self.tier == Tier::K,
            verdict: self.verdict(),
            rounds: self.rounds,
            truncated: self.truncated,
            keys_total: self.table.len() as u64,
            keys_in_start: self.n_start as u64,
            keys_outside: (self.table.len() - self.n_start) as u64,
            edges: self.edges,
            terms: self.terms.clone(),
            requires_coefficient_zero: self.requires_zero,
            descent_violations: self.violations,
            positive_numerator_nonzero: self.pos_nonzero,
            unrepresentable: self.unrepresentable,
            cycles: self.cycles,
            longest_chain: self.chain.clone(),
            undecided: self.undecided.clone(),
            open_total: self.open.len() as u64,
            open_in_start: open_in,
            open_outside: self.open.len() as u64 - open_in,
            open_by_status: by_status,
            open_by_face: by_face,
            credited: self.credits.len() as u64,
            credited_in_start: self
                .credits
                .keys()
                .filter(|&&i| self.start_mask[i as usize])
                .count() as u64,
            used_rules: fires.iter().filter(|f| f.0 > 0).count() as u64,
            used_rules_in_start: fires.iter().filter(|f| f.1 > 0).count() as u64,
            wins_per_table: wins,
            hand_downs,
            fact_requests: self.fact_requests().len() as u64,
            provisional: self.inputs.iter().any(|x| x.provisional_input()),
            inputs: self.inputs.clone(),
            terminals_preferred: self
                .terminals
                .values()
                .filter(|t| t.0 == TerminalKind::Preferred)
                .count() as u64,
            terminals_other: self
                .terminals
                .values()
                .filter(|t| t.0 == TerminalKind::Other)
                .count() as u64,
            terminals_in_start: self
                .terminals
                .keys()
                .filter(|&&i| self.start_mask[i as usize])
                .count() as u64,
            fact_source: self.fact_label.clone(),
            facts_exact: self.facts_exact,
            dispatch_order_digest: self.order_digest.clone(),
            start_set_digest: self.start_digest.clone(),
        }
    }

    fn lines_mask(&self) -> Vec<bool> {
        self.lines.clone()
    }

    /// The report artefact (timing left `None`: the caller may add it; it is not part of the identity).
    pub fn report(&self, id: &ReportIdentity) -> ClosureReport {
        let open = self.open_keys();
        let mut sha = sha256::Sha256::new();
        for o in &open {
            sha.update(serde_json::to_string(o).expect("serialise").as_bytes());
            sha.update(b"\n");
        }
        let mut config = BTreeMap::new();
        config.insert("bits".into(), self.codec.bits().to_string());
        config.insert("schema".into(), CLOSURE_SCHEMA.into());
        config.insert("crate_version".into(), env!("CARGO_PKG_VERSION").into());
        ClosureReport {
            schema: CLOSURE_SCHEMA.into(),
            identity: id.clone(),
            config,
            summary: self.summary(),
            descent_witnesses: self.descent_witnesses(),
            failure_witnesses: self.failure_witnesses(),
            open_keys: if open.len() <= 100_000 { open } else { vec![] },
            open_keys_digest: format!("sha256:{}", sha.finish_hex()),
            credits: self.credits(),
            terminals: self.terminal_records(),
            hand_down_digests: self
                .hand_down_keys
                .iter()
                .map(|(&m, k)| (m, k.digest()))
                .collect(),
            used_rules: self.used_rules(),
            timing: None,
        }
    }
}

/// Narrow view of the closure context for the symmetry module.
pub(crate) mod internal {
    use super::*;
    pub(crate) trait CtxView {
        fn dispatch(&self, key: &[i32]) -> Dispatch;
        fn position(&self, r: RuleRef) -> u32;
        fn rule_id(&self, pos: u32) -> RuleId<'_>;
        fn instantiate(
            &self,
            pos: u32,
            at: &[i32],
            out: &mut Vec<i32>,
            cm: &mut Vec<u64>,
        ) -> Result<Option<usize>, ClosureError>;
        fn sectors(&self) -> &Sectors;
        fn cmp(&self) -> &NodeComparator;
        fn facts(&self) -> &dyn FactSource;
        fn arity(&self) -> usize;
        fn require_regularity(&self) -> bool;
    }
    impl CtxView for Ctx<'_> {
        fn dispatch(&self, key: &[i32]) -> Dispatch {
            let (mut b, mut s) = (vec![], vec![]);
            self.fast.first_match_with(key, &mut b, &mut s)
        }
        fn position(&self, r: RuleRef) -> u32 {
            self.fast.position(r)
        }
        fn rule_id(&self, pos: u32) -> RuleId<'_> {
            self.rid(pos)
        }
        fn instantiate(
            &self,
            pos: u32,
            at: &[i32],
            out: &mut Vec<i32>,
            cm: &mut Vec<u64>,
        ) -> Result<Option<usize>, ClosureError> {
            Ok(self
                .bodies
                .instantiate(self.disp, self.fast.rule(pos), at, out, cm)?)
        }
        fn sectors(&self) -> &Sectors {
            &self.sectors
        }
        fn cmp(&self) -> &NodeComparator {
            self.cmp
        }
        fn facts(&self) -> &dyn FactSource {
            self.facts
        }
        fn arity(&self) -> usize {
            self.arity
        }
        fn require_regularity(&self) -> bool {
            self.cfg.require_regularity
        }
    }

    /// Context for the public `serve_by_image` (outside a closure run).
    pub(crate) struct StandaloneCtx<'a> {
        pub(crate) fast: FastDispatch<'a>,
        pub(crate) disp: &'a Dispatcher,
        pub(crate) bodies: &'a RuleBodies,
        pub(crate) cmp: &'a NodeComparator,
        pub(crate) sectors: Sectors,
        pub(crate) facts: &'a dyn FactSource,
        pub(crate) names: Vec<RuleName>,
        pub(crate) require_regularity: bool,
    }
    impl CtxView for StandaloneCtx<'_> {
        fn dispatch(&self, key: &[i32]) -> Dispatch {
            let (mut b, mut s) = (vec![], vec![]);
            self.fast.first_match_with(key, &mut b, &mut s)
        }
        fn position(&self, r: RuleRef) -> u32 {
            self.fast.position(r)
        }
        fn rule_id(&self, pos: u32) -> RuleId<'_> {
            let r = &self.names[pos as usize];
            RuleId {
                table: &r.table,
                record: r.record,
                label: r.label,
            }
        }
        fn instantiate(
            &self,
            pos: u32,
            at: &[i32],
            out: &mut Vec<i32>,
            cm: &mut Vec<u64>,
        ) -> Result<Option<usize>, ClosureError> {
            Ok(self
                .bodies
                .instantiate(self.disp, self.fast.rule(pos), at, out, cm)?)
        }
        fn sectors(&self) -> &Sectors {
            &self.sectors
        }
        fn cmp(&self) -> &NodeComparator {
            self.cmp
        }
        fn facts(&self) -> &dyn FactSource {
            self.facts
        }
        fn arity(&self) -> usize {
            self.cmp.arity()
        }
        fn require_regularity(&self) -> bool {
            self.require_regularity
        }
    }
}

/// Refusals of a symmetry credit: (group element index, reason).
pub type CreditRefusals = Vec<(usize, Refusal)>;

/// A credit of a previous closure that no longer holds (R44): the caller records `credit_invalidated`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvalidatedCredit {
    pub credit: CreditRecord,
    /// `covered_directly` (a rule now serves the key), `credited_differently` (another element / image / rule),
    /// `refused` (the key is open now), `terminal` (declared terminal), `not_reached`.
    pub reason: String,
}

/// Credits of `prev` invalidated in `new` (by a table-set change: a rule retired, superseded, or a changed winner at
/// the image), and credits that are new in `new`. Both closures must use the same key codec.
pub fn credit_diff(prev: &Closure, new: &Closure) -> (Vec<InvalidatedCredit>, Vec<CreditRecord>) {
    let old = prev.credits();
    let now = new.credits();
    let mut inval = vec![];
    for c in &old {
        let reason = match new.index_of(&c.key) {
            None => Some("not_reached"),
            Some(i) => {
                if new.is_credited(i) {
                    let n = now.iter().find(|x| x.key == c.key).expect("credited key");
                    (n.element != c.element || n.image != c.image || n.rule != c.rule)
                        .then_some("credited_differently")
                } else if new.is_open(i) {
                    Some("refused")
                } else if new.is_terminal(i) {
                    Some("terminal")
                } else {
                    Some("covered_directly")
                }
            }
        };
        if let Some(r) = reason {
            inval.push(InvalidatedCredit {
                credit: c.clone(),
                reason: r.into(),
            });
        }
    }
    let created = now
        .into_iter()
        .filter(|n| {
            !old.iter().any(|c| {
                c.key == n.key && c.element == n.element && c.image == n.image && c.rule == n.rule
            })
        })
        .collect();
    (inval, created)
}
