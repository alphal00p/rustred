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
pub mod relations;
pub mod sha256;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
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
pub use relations::{
    DispatchSlot, RelationSet, RelationStore, RelationStoreBuilder, VERDICT_CERTIFIED,
    VERDICT_DESCENDS, VERDICT_EXACT, VERDICT_REGULAR,
};

pub const CLOSURE_SCHEMA: &str = "rustred.campaign.closure.v1";
/// Version of the closure semantics recorded in every configuration identity (bump on any change of a result).
pub const CLOSURE_SEMANTICS: &str = "closure-semantics.v2";

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
    /// DEPRECATED (audit loop-A C3): `false` is the diagnostic relaxation `allow_unknown_regularity` (the run is
    /// stamped diagnostic and can never be `Closed`). Keep `true`; use [`ClosureConfig::diagnostic`] instead.
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
    /// Content digest of each table's COEFFICIENT file (table name -> digest, e.g. "sha256:.."): part of every rule's
    /// content digest (the fact identity) and of the configuration identity. A Tier C run in which a table without a
    /// digest fires is at best `closed_provisional` (blocking `table_coefficient_digest_missing:<name>`).
    pub table_digests: BTreeMap<String, String>,
    /// Source of `zero_sectors` (e.g. "engine-listing:sha256:..", "file:sha256:.."): REQUIRED when `zero_sectors` is
    /// given (typed error otherwise); recorded in the configuration identity. Without `zero_sectors` no sector is
    /// treated as zero (recorded as such).
    pub zero_sector_source: Option<String>,
    /// Rule-level key exclusions (plan 2.4.7 D7): at `key` the dispatch skips the named record and continues with the
    /// next (the importer's semantics). A declared terminal must be excluded from every rule that would serve it.
    pub exclusions: Vec<KeyExclusion>,
    /// Diagnostic relaxations: any set relaxation stamps the run `diagnostic` (verdict [`Verdict::Diagnostic`], never
    /// `Closed`), with the relaxed checks listed.
    pub diagnostic: DiagnosticRelaxations,
    /// Relation sets (R48 recipe rules, closure.md section 12): dispatch entries beside the tables, each with its slot.
    /// A set MATCHES a key iff the key is in its domain and in its certified set; otherwise the dispatch falls through.
    pub relation_sets: Vec<Arc<RelationSet>>,
}

/// Checks a DIAGNOSTIC run may relax (never in a run whose verdict is used; audit loop-A C3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticRelaxations {
    /// Tier C: an Unknown regularity (direct key or credit image) is not counted undecided.
    pub allow_unknown_regularity: bool,
}

/// Why a rule is excluded at a key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    /// The key is kept as a (declared / preferred) terminal.
    KeepsTerminal,
    Other(String),
}

/// A rule-level key exclusion: at `key`, the record (`table` name, `record` position, `label`, all checked) is not
/// considered by the dispatch.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct KeyExclusion {
    pub key: Vec<i32>,
    pub table: String,
    pub record: usize,
    pub label: i64,
    pub reason: ExclusionReason,
}

/// The configuration identity of a closure run (audit loop-A M2, M3, M6): one digest per input component the result
/// depends on, and the digest over all of them. Stored in every report and dispatch cache.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigIdentity {
    pub components: BTreeMap<String, String>,
}

impl ConfigIdentity {
    /// The digest over the components, optionally leaving some out.
    pub fn digest_without(&self, skip: &[&str]) -> String {
        let mut h = sha256::Sha256::new();
        h.update(b"rustred.campaign.closure-config.v1\n");
        for (k, v) in &self.components {
            if skip.contains(&k.as_str()) {
                continue;
            }
            h.update(k.as_bytes());
            h.update(b"=");
            h.update(v.as_bytes());
            h.update(b"\n");
        }
        format!("sha256:{}", h.finish_hex())
    }
    pub fn digest(&self) -> String {
        self.digest_without(&[])
    }
    /// Components that differ (present in either), leaving `skip` out.
    pub fn differing(&self, other: &ConfigIdentity, skip: &[&str]) -> Vec<String> {
        let mut keys: BTreeSet<&String> = self.components.keys().collect();
        keys.extend(other.components.keys());
        keys.into_iter()
            .filter(|k| !skip.contains(&k.as_str()))
            .filter(|k| self.components.get(*k) != other.components.get(*k))
            .cloned()
            .collect()
    }
}

/// Components that may differ between a closure and its [`closure_recheck`] continuation.
pub const RECHECK_MAY_DIFFER: &[&str] = &["start", "inputs"];
/// Components left out of the identity a fact request is bound to.
pub const FACT_BINDING_SKIPS: &[&str] = &["facts", "start", "inputs"];

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

/// The dispatch verdict at a declared terminal (with the key exclusions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalDispatch {
    Uncovered,
    OutsideChart,
    Covered,
    Blocked,
    Undecided,
    /// A relation set (recipe) serves the key.
    Recipe,
}

/// A declared terminal reached by the closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalRecord {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub kind: TerminalKind,
    /// The rule the dispatch (with the key exclusions) stops at there (None: uncovered / outside the chart).
    pub would_dispatch: Option<RuleName>,
    pub dispatch: TerminalDispatch,
    /// True if the shipped table would NOT stop at this terminal (the dispatch selects a rule there and no exclusion
    /// removes it): the closure is then `terminal_in_rule_domain`, never closed (audit loop-A M1).
    pub in_rule_domain: bool,
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
            table_digests: BTreeMap::new(),
            zero_sector_source: None,
            exclusions: vec![],
            diagnostic: DiagnosticRelaxations::default(),
            relation_sets: vec![],
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
    /// The previous dispatch cache / closure cannot be reused (the message names the differing components).
    NotIncremental(String),
    /// Inconsistent configuration (unknown exclusion rule, malformed credit group, ...).
    Config(String),
    /// A required input label is missing (e.g. a zero-sector list without its source).
    MissingInput(String),
    /// More reached keys than the u32 key index allows.
    Capacity(String),
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
    /// A relation set serves the key but its record's verdict bits are not all set.
    RecipeUnverified,
}

/// One open key of the closure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenKey {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub status: OpenStatus,
    pub rule: Option<RuleName>,
    pub detail: String,
    /// Relation sets whose DOMAIN contains the key (uncovered keys; information for the work list).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recipe_candidates: Vec<String>,
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
    /// A declared (or preferred) terminal is the first match of a rule without a key exclusion: the shipped table
    /// would not stop there (audit loop-A M1). Listed in the report's `terminals` with `in_rule_domain: true`.
    TerminalInRuleDomain,
    /// A diagnostic run (some check relaxed, listed in `relaxed_checks`): never a closure verdict.
    Diagnostic,
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
    /// Declared terminals inside a rule's domain without an exclusion.
    pub terminal_in_rule_domain: u64,
    /// True if some check was relaxed (verdict `diagnostic`).
    pub diagnostic: bool,
    pub relaxed_checks: Vec<String>,
    /// The verdict the run would have without its diagnostic stamp (informational; `None` if not diagnostic).
    pub verdict_if_not_diagnostic: Option<Verdict>,
    /// Every reason the verdict is not `closed` (empty iff `closed`).
    pub blocking: Vec<String>,
    /// The configuration identity digest (all inputs; see `ClosureReport::config`).
    pub config_digest: String,
    /// Whether the exit (hand-down) key sets are complete for THIS configuration (hand-down mode `keys`, not
    /// truncated).
    pub exits_complete: bool,
    /// The configuration identity the fact source was requested by (provenance), if any.
    pub facts_requested_by: Option<String>,
    /// Keys served by relation sets, and the number of sets used.
    pub recipe_served: u64,
    pub recipe_served_in_start: u64,
    pub used_relation_sets: u64,
}

/// Use of one relation set (R48): keys it serves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationUse {
    pub set: String,
    pub recipe_digest: String,
    pub keys: u64,
    pub keys_in_start: u64,
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
    /// The configuration identity components, plus `digest`, `schema`, `crate_version`.
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
    /// Relation sets used (keys served), as (recipe digest, key) uses per set.
    pub used_relations: Vec<RelationUse>,
    pub timing: Option<BTreeMap<String, f64>>,
}

// ------------------------------------------------------------------------------------------------ engine internals

const PENDING: u32 = u32::MAX - 2;
const CREDITED: u32 = u32::MAX - 1;
const OPEN: u32 = u32::MAX;
const TERMINAL: u32 = u32::MAX - 3;
const RECIPE: u32 = u32::MAX - 4;
/// Stop sentinels (per-key dispatch memo): not evaluated (truncated, outside the chart) / evaluated, no record stops.
const STOP_NONE: u32 = u32::MAX - 1;
const STOP_UNCOVERED: u32 = u32::MAX;

/// Compact fact request.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ReqC {
    key: u32,
    pos: u32,
    term: u32,
    reason: RequestReason,
    image: bool,
    /// The term's instantiated key (empty for regularity).
    rhs: Box<[i32]>,
}

#[derive(Clone, Debug)]
struct OpenInfo {
    status: OpenStatus,
    rule: Option<u32>,
    detail: String,
    candidates: Vec<u32>,
}

/// Per-key DISPATCH memo for the incremental closure (audit loop-A C1, M2): only the dispatch verdict (with the key
/// exclusions) is cached, never a disposition (terminal, credited, followed, open), which every run recomputes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CachedStop {
    /// Not evaluated (truncated, outside the chart): re-dispatched in full.
    NotEvaluated,
    /// Every record evaluated, none stopped.
    Uncovered,
    /// The stopping record (covered / blocked / undecided): (table index in the cached dispatcher, record).
    At(u32, u32),
}

/// The dispatch memo of a closure run, with the dispatch identity it is valid for.
#[derive(Clone, Debug)]
pub struct DispatchCache {
    arity: usize,
    bits: u32,
    chart: Option<Vec<bool>>,
    /// per table (dispatcher index): (name, dispatch content digest)
    tables: Vec<(String, String)>,
    priority: Vec<usize>,
    /// per key with exclusions: sorted (table name, record)
    exclusions: HashMap<Vec<i32>, Vec<(String, usize)>>,
    table: KeyTable,
    stop: Vec<CachedStop>,
    /// The full configuration identity of the run that produced the cache (provenance).
    pub config: ConfigIdentity,
}

impl DispatchCache {
    pub fn len(&self) -> usize {
        self.table.len()
    }
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
}

/// How an incremental run used its cache.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct IncrementalUse {
    /// Keys whose cached dispatch was reused (only records of changed / new tables, and records at or after the
    /// cached stop, evaluated).
    pub reused: u64,
    /// Keys dispatched in full (not in the cache, stop in a changed table, exclusions changed, not evaluated).
    pub full: u64,
    /// Why the cache could not be used at all (`None`: used).
    pub fallback: Option<String>,
    /// Tables of the new dispatcher identical in dispatch content to a cached table.
    pub unchanged_tables: Vec<String>,
}

/// The exit (hand-down) key sets of a run, tied to the run's configuration identity.
#[derive(Clone, Debug)]
pub struct ExitSets<'a> {
    /// True iff the sets are complete for this configuration (hand-down mode `keys`, not truncated).
    pub complete: bool,
    pub config_digest: String,
    pub sets: &'a BTreeMap<u64, KeySet>,
}

/// A reached declared terminal (internal).
#[derive(Clone, Copy, Debug)]
struct TermInfo {
    kind: TerminalKind,
    would: Option<u32>,
    dispatch: TerminalDispatch,
}

impl TermInfo {
    fn in_rule_domain(&self) -> bool {
        matches!(
            self.dispatch,
            TerminalDispatch::Covered
                | TerminalDispatch::Blocked
                | TerminalDispatch::Undecided
                | TerminalDispatch::Recipe
        )
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
    terminals: BTreeMap<u32, TermInfo>,
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
    rule_digests: Vec<String>,
    fact_tables: Vec<String>,
    table_names: Vec<String>,
    entry_table: Vec<u32>,
    fact_label: String,
    facts_exact: bool,
    facts_requested_by: Option<String>,
    order_digest: String,
    start_digest: String,
    lines: Vec<bool>,
    identity: ConfigIdentity,
    relaxed: Vec<String>,
    hand_downs_mode: HandDowns,
    /// tables (names) without a coefficient-file digest
    unbound_tables: BTreeSet<String>,
    // dispatch identity (for the cache)
    disp_arity: usize,
    disp_chart: Option<Vec<bool>>,
    disp_tables: Vec<(String, String)>,
    disp_priority: Vec<usize>,
    excl_names: HashMap<Vec<i32>, Vec<(String, usize)>>,
    incremental: Option<IncrementalUse>,
    /// recipe-served key -> (relation set, record index)
    recipe_of: BTreeMap<u32, (u32, u32)>,
    rset_names: Vec<(String, String)>,
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
    rule_digests: Vec<String>,
    fact_tables: Vec<String>,
    /// key -> excluded dispatch positions (sorted)
    exclusions: HashMap<Vec<i32>, Vec<u32>>,
    term_pref: Option<KeySet>,
    term_other: Option<KeySet>,
    relax_regularity: bool,
    /// relation sets in dispatch order with their rank (= priority rank of the table they precede; tables count)
    rsets: Vec<(u32, Arc<RelationSet>)>,
    /// table index -> priority rank
    table_rank: Vec<u32>,
    /// key -> excluded relation sets (indices into `rsets`)
    excl_sets: HashMap<Vec<i32>, Vec<usize>>,
}

impl Ctx<'_> {
    fn rid(&self, pos: u32) -> RuleId<'_> {
        let r = &self.rule_names[pos as usize];
        RuleId {
            table: &self.fact_tables[pos as usize],
            record: r.record,
            label: r.label,
            digest: &self.rule_digests[pos as usize],
        }
    }
    fn excluded(&self, key: &[i32]) -> &[u32] {
        if self.exclusions.is_empty() {
            return &[];
        }
        self.exclusions
            .get(key)
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }
    /// The relation set serving `key` given the table dispatch `d` (the first matching set ranked before d's stop).
    fn recipe_match(&self, key: &[i32], d: &Dispatch) -> Option<(u32, u32)> {
        if self.rsets.is_empty() {
            return None;
        }
        let rd = match d {
            Dispatch::Covered(r) => self.table_rank[r.table],
            Dispatch::Blocked { at, .. } | Dispatch::Undecided { at, .. } => {
                self.table_rank[at.table]
            }
            Dispatch::Uncovered => u32::MAX,
            Dispatch::OutsideChart { .. } => return None,
        };
        let ex = self.excl_sets.get(key);
        for (si, (rank, set)) in self.rsets.iter().enumerate() {
            if *rank > rd {
                break;
            }
            if ex.is_some_and(|v| v.contains(&si)) {
                continue;
            }
            if let Some(i) = set.relations.find(key)
                && set.in_domain(key)
            {
                return Some((si as u32, i as u32));
            }
        }
        None
    }
    fn recipe_candidates(&self, key: &[i32]) -> Vec<u32> {
        self.rsets
            .iter()
            .enumerate()
            .filter(|(_, (_, s))| s.in_domain(key))
            .map(|(i, _)| i as u32)
            .collect()
    }
    fn terminal_kind(&self, key: &[i32]) -> Option<TerminalKind> {
        if self.term_pref.as_ref().is_some_and(|t| t.contains(key)) {
            Some(TerminalKind::Preferred)
        } else if self.term_other.as_ref().is_some_and(|t| t.contains(key)) {
            Some(TerminalKind::Other)
        } else {
            None
        }
    }
}

/// Per-position rule names and content digests, per-table dispatch and body digests (closure.md section 11).
struct RuleIds {
    names: Vec<RuleName>,
    digests: Vec<String>,
    table_dispatch: Vec<String>,
    table_bodies: Vec<String>,
}

fn rule_ids(
    disp: &Dispatcher,
    bodies: &RuleBodies,
    fast: &FastDispatch,
    source_digest: &dyn Fn(&str) -> Option<String>,
    name_of: &dyn Fn(usize) -> String,
) -> RuleIds {
    let tabs = disp.tables();
    let mut rec_digest: Vec<Vec<String>> = Vec::with_capacity(tabs.len());
    let mut table_dispatch = Vec::with_capacity(tabs.len());
    let mut table_bodies = Vec::with_capacity(tabs.len());
    for (t, tab) in tabs.iter().enumerate() {
        let src = source_digest(&tab.name).unwrap_or_else(|| "unrecorded".into());
        let mut hd = sha256::Sha256::new();
        hd.update(b"rustred.campaign.table-dispatch.v1\n");
        hd.update(&(tab.len() as u64).to_le_bytes());
        let mut hb = sha256::Sha256::new();
        hb.update(b"rustred.campaign.table-bodies.v1\n");
        let mut per = Vec::with_capacity(tab.len());
        for r in 0..tab.len() {
            let dom = format!("{:?}", tab.domains[r]);
            hd.update(&tab.labels[r].to_le_bytes());
            hd.update(&(dom.len() as u64).to_le_bytes());
            hd.update(dom.as_bytes());
            let mut hr = sha256::Sha256::new();
            hr.update(b"rustred.campaign.rule.v1\n");
            hr.update(src.as_bytes());
            hr.update(b"\n");
            hr.update(&(r as u64).to_le_bytes());
            hr.update(&tab.labels[r].to_le_bytes());
            hr.update(&(dom.len() as u64).to_le_bytes());
            hr.update(dom.as_bytes());
            let mut hbr = sha256::Sha256::new();
            bodies.digest_record(t, r, &mut hbr);
            let bd = hbr.finish_hex();
            hb.update(bd.as_bytes());
            hr.update(bd.as_bytes());
            per.push(format!("sha256:{}", hr.finish_hex()));
        }
        table_dispatch.push(format!("sha256:{}", hd.finish_hex()));
        table_bodies.push(format!("sha256:{}", hb.finish_hex()));
        rec_digest.push(per);
    }
    let n = fast.n_entries() as u32;
    let mut names = Vec::with_capacity(n as usize);
    let mut digests = Vec::with_capacity(n as usize);
    for p in 0..n {
        let r = fast.rule(p);
        names.push(RuleName {
            table: name_of(r.table),
            record: r.record,
            label: r.label,
        });
        digests.push(rec_digest[r.table][r.record].clone());
    }
    RuleIds {
        names,
        digests,
        table_dispatch,
        table_bodies,
    }
}

/// Rule names (table = `provenance.source`) and content digests (no coefficient-file digest) for the standalone
/// credit checker.
pub(crate) fn standalone_rule_ids(
    disp: &Dispatcher,
    bodies: &RuleBodies,
    fast: &FastDispatch,
) -> (Vec<RuleName>, Vec<String>) {
    let ids = rule_ids(disp, bodies, fast, &|_| None, &|t| {
        disp.tables()[t].provenance.source.clone()
    });
    (ids.names, ids.digests)
}

fn sha_hex(parts: &[&str]) -> String {
    let mut h = sha256::Sha256::new();
    for p in parts {
        h.update(&(p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    format!("sha256:{}", h.finish_hex())
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

/// Expand a key served by a relation set: its RECORDED right-hand-side keys (exact at the key: no facts needed).
/// Top keys are edges (descent checked); a removed line is a hand-down; a positive numerator key is a failure (the
/// recorded right-hand side omits vanishing terms, so such a term is present with a non-zero coefficient).
fn expand_recipe(
    ctx: &Ctx,
    si: u32,
    ri: u32,
    key: &[i32],
    buf: &mut Vec<i32>,
    codes: &mut Vec<u64>,
    sink: &mut Sink,
) {
    let set = &ctx.rsets[si as usize].1;
    let st = &set.relations;
    let w = ctx.codec.words();
    let sw = st.codec().words();
    codes.clear();
    buf.resize(ctx.arity, 0);
    let rname = || RuleName {
        table: set.name.clone(),
        record: ri as usize,
        label: -1,
    };
    let cap = ctx.cfg.witness_cap;
    let mut code = [0u64; keyset::MAX_WORDS];
    for (j, c) in st.rhs_codes(ri as usize).chunks_exact(sw).enumerate() {
        st.codec().decode(c, buf);
        let t: &[i32] = buf;
        let class = ctx.sectors.classify_masked(t, 0);
        let enc = ctx.codec.encode(t, &mut code);
        match sink {
            Sink::Bfs { out, key_idx, .. } => {
                out.terms.total += 1;
                let wit = |reason: &str| Witness {
                    key: key.to_vec(),
                    rule: rname(),
                    term: j,
                    rhs: t.to_vec(),
                    reason: reason.into(),
                };
                match class {
                    TermClass::PositiveNumerator { subsector, .. } => {
                        if subsector {
                            out.terms.positive_numerator_sub += 1;
                        } else {
                            out.terms.positive_numerator_top += 1;
                        }
                        out.pos_nonzero += 1;
                        push_capped(
                            &mut out.failure_w,
                            (*key_idx, j as u32, wit("recipe_positive_numerator")),
                            cap,
                        );
                    }
                    TermClass::ZeroSector { .. } => out.terms.zero_sector += 1,
                    TermClass::Subsector { dropped } => {
                        out.terms.subsector += 1;
                        match ctx.cfg.hand_downs {
                            HandDowns::Off => {}
                            HandDowns::Counts => *out.hd_terms.entry(dropped).or_default() += 1,
                            HandDowns::Keys => {
                                *out.hd_terms.entry(dropped).or_default() += 1;
                                if enc {
                                    out.hd_codes
                                        .entry(dropped)
                                        .or_default()
                                        .extend_from_slice(&code[..w]);
                                } else {
                                    out.unrepresentable += 1;
                                    push_capped(
                                        &mut out.failure_w,
                                        (*key_idx, j as u32, wit("unrepresentable_hand_down")),
                                        cap,
                                    );
                                }
                            }
                        }
                    }
                    TermClass::Top => {
                        out.terms.top += 1;
                        if !ctx.cmp.lower(t, key) {
                            out.violations += 1;
                            push_capped(
                                &mut out.descent_w,
                                (*key_idx, j as u32, wit("descent_violation")),
                                cap,
                            );
                        }
                        if enc {
                            codes.extend_from_slice(&code[..w]);
                        } else {
                            out.unrepresentable += 1;
                            push_capped(
                                &mut out.failure_w,
                                (*key_idx, j as u32, wit("unrepresentable_key")),
                                cap,
                            );
                        }
                    }
                }
            }
            Sink::Succ { .. } => {
                if class == TermClass::Top && enc {
                    codes.extend_from_slice(&code[..w]);
                }
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
            ctx.facts.coefficient(&ctx.rid(pos), applied, j, t)
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
                            rhs: t.into(),
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
                                rhs: t.into(),
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
    Terminal(TermInfo),
    Recipe(u32, u32),
    Pos(u32, Option<RegFact>),
    Credit(Box<Credit>, Option<RegFact>),
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
/// computed is kept. ONLY the components [`RECHECK_MAY_DIFFER`] (`start`, `inputs`) may differ from `prev`'s
/// configuration identity; any other differing component (tables, bodies, comparator, facts, terminals,
/// exclusions, credits, zero sectors, hand-down mode, tier, codec, ...) is refused with `NotIncremental` naming the
/// components (audit loop-A M3); a truncated `prev` is refused. The result equals a from-scratch closure of
/// (previous start set + `extra`) (tested, property test). The labels in `cfg.inputs` are added to the previous ones.
pub fn closure_recheck(
    prev: &Closure,
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    extra: &KeySet,
    cfg: &ClosureConfig,
    facts: &dyn FactSource,
) -> Result<Closure, ClosureError> {
    if prev.truncated {
        return Err(ClosureError::NotIncremental(
            "previous closure was truncated".into(),
        ));
    }
    run(disp, bodies, cmp, extra, cfg, facts, None, Some(prev))
}

/// Incremental closure: as [`closure`], reusing ONLY the per-key DISPATCH verdicts of `prev` where they are provably
/// unchanged (closure.md section 6): a cached key is re-evaluated on every record of a table that is new or changed in
/// dispatch content (and on its stopping record and everything after it); keys whose stop lies in a changed / removed
/// table, whose exclusions changed, that were not evaluated, or that are new, are dispatched in full. Every other
/// component (terminals, exclusions, credits, facts, bodies, zero sectors, hand-down mode, start set, envelopes) is
/// recomputed in this run, so any of them may differ. A different arity or codec width is refused (`NotIncremental`);
/// a different chart or a changed relative order of the unchanged tables falls back to a full dispatch (recorded in
/// [`Closure::incremental_use`]). The result is identical to [`closure`] (property test).
pub fn closure_incremental(
    prev: &DispatchCache,
    disp: &Dispatcher,
    bodies: &RuleBodies,
    cmp: &NodeComparator,
    start: &KeySet,
    cfg: &ClosureConfig,
    facts: &dyn FactSource,
) -> Result<Closure, ClosureError> {
    if prev.arity != disp.arity() || prev.bits != cfg.bits {
        return Err(ClosureError::NotIncremental(format!(
            "cache of arity {} / {} bits, run of arity {} / {} bits",
            prev.arity,
            prev.bits,
            disp.arity(),
            cfg.bits
        )));
    }
    run(disp, bodies, cmp, start, cfg, facts, Some(prev), None)
}

/// The view of a dispatch cache in the new dispatcher's positions.
struct CacheView<'c> {
    cache: &'c DispatchCache,
    /// old table index -> new table index (unchanged tables only)
    old_to_new: Vec<Option<usize>>,
    /// per new table: identical in dispatch content to a cached table
    reusable_table: Vec<bool>,
}

fn cache_view<'c>(
    cache: &'c DispatchCache,
    disp: &Dispatcher,
    table_dispatch: &[String],
    use_: &mut IncrementalUse,
) -> Option<CacheView<'c>> {
    let tabs = disp.tables();
    if cache.chart.as_deref() != disp.chart() {
        use_.fallback = Some("chart differs".into());
        return None;
    }
    fn names_unique<'x>(v: impl Iterator<Item = &'x String>) -> bool {
        let mut seen = BTreeSet::new();
        v.into_iter().all(|n| seen.insert(n))
    }
    if !names_unique(cache.tables.iter().map(|t| &t.0))
        || !names_unique(tabs.iter().map(|t| &t.name))
    {
        use_.fallback = Some("duplicate table names".into());
        return None;
    }
    let mut old_to_new = vec![None; cache.tables.len()];
    let mut reusable_table = vec![false; tabs.len()];
    for (i, (name, dig)) in cache.tables.iter().enumerate() {
        if let Some(j) = tabs.iter().position(|t| t.name == *name)
            && table_dispatch[j] == *dig
        {
            old_to_new[i] = Some(j);
            reusable_table[j] = true;
            use_.unchanged_tables.push(name.clone());
        }
    }
    // relative order of the unchanged tables must be kept
    let rank_new: Vec<usize> = {
        let mut r = vec![0; tabs.len()];
        for (k, &t) in disp.priority().iter().enumerate() {
            r[t] = k;
        }
        r
    };
    let seq: Vec<usize> = cache
        .priority
        .iter()
        .filter_map(|&i| old_to_new[i].map(|j| rank_new[j]))
        .collect();
    if seq.windows(2).any(|w| w[0] > w[1]) {
        use_.fallback = Some("relative order of the unchanged tables changed".into());
        use_.unchanged_tables.clear();
        return None;
    }
    Some(CacheView {
        cache,
        old_to_new,
        reusable_table,
    })
}

fn hand_downs_name(h: HandDowns) -> &'static str {
    match h {
        HandDowns::Off => "off",
        HandDowns::Counts => "counts",
        HandDowns::Keys => "keys",
    }
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
    if cfg.zero_sectors.is_some() && cfg.zero_sector_source.is_none() {
        return Err(ClosureError::MissingInput(
            "zero_sectors given without zero_sector_source (audit loop-A M6)".into(),
        ));
    }
    if let Some(pol) = &cfg.credits {
        pol.validate(&lines).map_err(ClosureError::Config)?;
    }
    let codec = KeyCodec::new(arity, cfg.bits)?;
    let fast = FastDispatch::new(disp);
    let n_entries = fast.n_entries();
    let ids = rule_ids(
        disp,
        bodies,
        &fast,
        &|name| cfg.table_digests.get(name).cloned(),
        &|t| disp.tables()[t].name.clone(),
    );
    let entry_table: Vec<u32> = (0..n_entries as u32)
        .map(|p| fast.rule(p).table as u32)
        .collect();
    // ---- exclusions
    let mut exclusions: HashMap<Vec<i32>, Vec<u32>> = HashMap::new();
    let mut excl_names: HashMap<Vec<i32>, Vec<(String, usize)>> = HashMap::new();
    // relation sets: rank and order
    let table_rank: Vec<u32> = {
        let mut r = vec![0u32; disp.tables().len()];
        for (k, &t) in disp.priority().iter().enumerate() {
            r[t] = k as u32;
        }
        r
    };
    let mut rsets: Vec<(u32, Arc<RelationSet>)> = vec![];
    for rs in &cfg.relation_sets {
        if rs.relations.codec().arity() != arity || rs.domain.arity != arity {
            return Err(ClosureError::KeyArity {
                expected: arity,
                got: rs.relations.codec().arity(),
            });
        }
        let rank = match &rs.slot {
            DispatchSlot::Last => disp.tables().len() as u32,
            DispatchSlot::BeforeTable(n) => {
                let t = disp
                    .tables()
                    .iter()
                    .position(|t| t.name == *n)
                    .ok_or_else(|| {
                        ClosureError::Config(format!("relation set {}: no table {n}", rs.name))
                    })?;
                table_rank[t]
            }
        };
        if rsets.iter().any(|(_, x)| x.name == rs.name)
            || disp.tables().iter().any(|t| t.name == rs.name)
        {
            return Err(ClosureError::Config(format!(
                "relation set name {} is not unique",
                rs.name
            )));
        }
        rsets.push((rank, rs.clone()));
    }
    rsets.sort_by_key(|x| x.0); // stable: caller order within a rank
    let mut excl_sets: HashMap<Vec<i32>, Vec<usize>> = HashMap::new();
    for e in &cfg.exclusions {
        if e.key.len() != arity {
            return Err(ClosureError::KeyArity {
                expected: arity,
                got: e.key.len(),
            });
        }
        if let Some(si) = rsets.iter().position(|(_, x)| x.name == e.table) {
            if e.record != 0 || e.label != 0 {
                return Err(ClosureError::Config(format!(
                    "exclusion of relation set {}: record and label must be 0",
                    e.table
                )));
            }
            excl_sets.entry(e.key.clone()).or_default().push(si);
            continue;
        }
        let mut found = disp
            .tables()
            .iter()
            .enumerate()
            .filter(|(_, t)| t.name == e.table);
        let (ti, tab) = found
            .next()
            .ok_or_else(|| ClosureError::Config(format!("exclusion: no table {}", e.table)))?;
        if found.next().is_some() {
            return Err(ClosureError::Config(format!(
                "exclusion: table name {} is not unique",
                e.table
            )));
        }
        if tab.labels.get(e.record) != Some(&e.label) {
            return Err(ClosureError::Config(format!(
                "exclusion: table {} has no record {} with label {}",
                e.table, e.record, e.label
            )));
        }
        let p = fast.position(RuleRef {
            table: ti,
            record: e.record,
            label: e.label,
        });
        exclusions.entry(e.key.clone()).or_default().push(p);
        excl_names
            .entry(e.key.clone())
            .or_default()
            .push((e.table.clone(), e.record));
    }
    for v in exclusions.values_mut() {
        v.sort_unstable();
        v.dedup();
    }
    for v in excl_names.values_mut() {
        v.sort();
        v.dedup();
    }
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
    let mut relaxed: Vec<String> = vec![];
    if cfg.diagnostic.allow_unknown_regularity || !cfg.require_regularity {
        relaxed.push("allow_unknown_regularity".into());
    }
    // ---- configuration identity (start / inputs filled at the end)
    let mut identity = ConfigIdentity::default();
    {
        let c = &mut identity.components;
        c.insert(
            "tool".into(),
            format!(
                "rustred-campaign-core {} {CLOSURE_SCHEMA} {CLOSURE_SEMANTICS}",
                env!("CARGO_PKG_VERSION")
            ),
        );
        c.insert("tier".into(), format!("{:?}", cfg.tier));
        c.insert("bits".into(), cfg.bits.to_string());
        c.insert(
            "lines".into(),
            lines.iter().map(|&l| if l { '1' } else { '0' }).collect(),
        );
        c.insert(
            "comparator".into(),
            sha_hex(&[&format!(
                "{}|{:?}|{:?}",
                cmp.name(),
                cmp.equivalent_descriptor(),
                cmp.presentation()
            )]),
        );
        let mut dparts = vec![format!("{:?}", disp.chart())];
        let mut bparts = vec![];
        let mut sparts = vec![];
        for (k, &t) in disp.priority().iter().enumerate() {
            let name = &disp.tables()[t].name;
            dparts.push(format!("{k}:{name}:{}", ids.table_dispatch[t]));
            bparts.push(format!("{name}:{}", ids.table_bodies[t]));
            sparts.push(format!(
                "{name}:{}",
                cfg.table_digests
                    .get(name)
                    .map(|x| x.as_str())
                    .unwrap_or("unrecorded")
            ));
        }
        let refs = |v: &Vec<String>| sha_hex(&v.iter().map(|x| x.as_str()).collect::<Vec<_>>());
        c.insert("dispatch".into(), refs(&dparts));
        c.insert("bodies".into(), refs(&bparts));
        c.insert("table_sources".into(), refs(&sparts));
        let ft: Vec<String> = cfg
            .fact_tables
            .iter()
            .map(|(a, b)| format!("{a}={b}"))
            .collect();
        c.insert("fact_tables".into(), refs(&ft));
        let mut ex: Vec<String> = cfg.exclusions.iter().map(|e| format!("{e:?}")).collect();
        ex.sort();
        ex.dedup();
        c.insert(
            "exclusions".into(),
            if ex.is_empty() {
                "none".into()
            } else {
                refs(&ex)
            },
        );
        let kd = |k: &Option<KeySet>| k.as_ref().map(|k| k.digest()).unwrap_or("none".into());
        c.insert(
            "terminals".into(),
            format!("preferred={};other={}", kd(&term_pref), kd(&term_other)),
        );
        c.insert(
            "credits".into(),
            match &cfg.credits {
                None => "none".into(),
                Some(p) => sha_hex(&[
                    &format!("{:?}", p.group),
                    &format!("{:?}", p.pure_numerator_maps),
                ]),
            },
        );
        c.insert(
            "zero_sectors".into(),
            match (&cfg.zero_sectors, &cfg.zero_sector_source) {
                (None, _) => "none (no sector treated as zero)".into(),
                (Some(z), src) => format!(
                    "{}|{}",
                    src.as_deref().unwrap_or("unlabelled"),
                    sha_hex(&[&format!("{z:?}")])
                ),
            },
        );
        let rparts: Vec<String> = rsets
            .iter()
            .map(|(rank, x)| {
                format!(
                    "{rank}:{}:{:?}:{}:{}:{}",
                    x.name,
                    x.slot,
                    x.recipe_digest,
                    sha_hex(&[&format!("{:?}", x.domain)]),
                    x.relations.digest()
                )
            })
            .collect();
        c.insert(
            "relations".into(),
            if rparts.is_empty() {
                "none".into()
            } else {
                refs(&rparts)
            },
        );
        c.insert("hand_downs".into(), hand_downs_name(cfg.hand_downs).into());
        c.insert("max_rounds".into(), format!("{:?}", cfg.max_rounds));
        c.insert(
            "diagnostic".into(),
            if relaxed.is_empty() {
                "none".into()
            } else {
                relaxed.join(",")
            },
        );
        c.insert(
            "facts".into(),
            format!("{}|exact={}", facts.label(), facts.exact()),
        );
    }
    if let Some(p) = resume {
        let diff = identity.differing(&p.identity, RECHECK_MAY_DIFFER);
        if !diff.is_empty() {
            return Err(ClosureError::NotIncremental(format!(
                "configuration components differ from the previous closure: {}",
                diff.join(", ")
            )));
        }
    }
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
        fact_tables: ids
            .names
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
        rule_names: ids.names,
        rule_digests: ids.digests,
        exclusions,
        term_pref,
        term_other,
        relax_regularity: !relaxed.is_empty(),
        rsets,
        table_rank,
        excl_sets,
    };
    // start keys (canonical order)
    let start = if *start.codec() == codec {
        start.clone()
    } else {
        start.recode(codec)?
    };
    // cache translation (incremental)
    let mut inc_use = IncrementalUse::default();
    let view = cache.and_then(|c| cache_view(c, disp, &ids.table_dispatch, &mut inc_use));
    let reusable = |p: u32| -> bool {
        view.as_ref()
            .is_some_and(|v| v.reusable_table[entry_table[p as usize] as usize])
    };

    let w = codec.words();
    let mut table;
    let mut res: Vec<u32>;
    let mut stop: Vec<u32>;
    let mut open: BTreeMap<u32, OpenInfo>;
    let mut credits: BTreeMap<u32, Credit>;
    let mut terminals: BTreeMap<u32, TermInfo>;
    let mut recipe_of: BTreeMap<u32, (u32, u32)>;
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
            recipe_of = BTreeMap::new();
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
            recipe_of = p.recipe_of.clone();
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
            stop.push(STOP_NONE);
            start_mask.push(true);
        } else {
            start_mask[ix as usize] = true;
        }
    }
    if resume.is_none() {
        lo = 0;
    }
    drop(start);
    let index_cap = TERMINAL as usize - 8;
    loop {
        let hi = table.len();
        if hi >= index_cap {
            return Err(ClosureError::Capacity(format!(
                "{hi} reached keys: u32 key index exhausted (audit loop-A m6)"
            )));
        }
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
                        candidates: vec![],
                    },
                );
            }
            break;
        }
        rounds += 1;
        // ---- dispatch (always, also at declared terminals), then the disposition
        let parts = chunks(lo, hi, cfg.threads);
        type DispOut = Vec<(KeyRes, u32, bool)>;
        let results: Vec<Result<DispOut, ClosureError>> = std::thread::scope(|s| {
            let hs: Vec<_> = parts
                .iter()
                .map(|&(a, b)| {
                    let (ctx, table, reusable, view) = (&ctx, &table, &reusable, &view);
                    s.spawn(move || -> Result<DispOut, ClosureError> {
                        let mut buf = vec![];
                        let mut st = vec![];
                        let mut key = vec![0i32; arity];
                        let mut v = Vec::with_capacity(b - a);
                        for i in a..b {
                            codec.decode(table.code(i), &mut key);
                            let excl = ctx.excluded(&key);
                            let mut used_cache = false;
                            let (old_stop, use_reuse) = match view {
                                None => (0, false),
                                Some(cv) => {
                                    let cached = cv
                                        .cache
                                        .table
                                        .get(table.code(i))
                                        .map(|ci| cv.cache.stop[ci as usize]);
                                    let same_excl = (cv.cache.exclusions.is_empty()
                                        && ctx.cfg.exclusions.is_empty())
                                        || cv.cache.exclusions.get(&key)
                                            == ctx_excl_names(ctx, &key).as_ref();
                                    match cached {
                                        Some(CachedStop::Uncovered) if same_excl => {
                                            used_cache = true;
                                            (u32::MAX, true)
                                        }
                                        Some(CachedStop::At(t, r)) if same_excl => {
                                            match cv.old_to_new[t as usize] {
                                                Some(j) => {
                                                    used_cache = true;
                                                    (
                                                        ctx.fast.position(RuleRef {
                                                            table: j,
                                                            record: r as usize,
                                                            label: 0,
                                                        }),
                                                        true,
                                                    )
                                                }
                                                None => (0, false),
                                            }
                                        }
                                        _ => (0, false),
                                    }
                                }
                            };
                            let d = ctx
                                .fast
                                .first_match_ex(
                                    &key,
                                    old_stop,
                                    if use_reuse { Some(reusable) } else { None },
                                    excl,
                                    &mut buf,
                                    &mut st,
                                )
                                .0;
                            let stp = stop_of(ctx, &d);
                            let rec = ctx.recipe_match(&key, &d);
                            v.push((dispose(ctx, &key, d, rec)?, stp, used_cache));
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
            for (kr, st, used_cache) in part? {
                stop[i] = st;
                if view.is_some() {
                    if used_cache {
                        inc_use.reused += 1;
                    } else {
                        inc_use.full += 1;
                    }
                }
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
                                rhs: Box::new([]),
                            });
                        }
                    }
                    KeyRes::Credit(c, reg) => {
                        res[i] = CREDITED;
                        if reg == Some(RegFact::Unknown) {
                            acc.undecided.regularity_unknown += 1;
                            acc.requests.push(ReqC {
                                key: i as u32,
                                pos: c.position,
                                term: u32::MAX,
                                reason: RequestReason::Regularity,
                                image: true,
                                rhs: Box::new([]),
                            });
                        }
                        credits.insert(i as u32, *c);
                    }
                    KeyRes::Open(o) => {
                        res[i] = OPEN;
                        open.insert(i as u32, o);
                    }
                    KeyRes::Terminal(t) => {
                        res[i] = TERMINAL;
                        terminals.insert(i as u32, t);
                    }
                    KeyRes::Recipe(si, ri) => {
                        res[i] = RECIPE;
                        recipe_of.insert(i as u32, (si, ri));
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
                    let (ctx, table, res, credits, recipe_of) =
                        (&ctx, &table, &res, &credits, &recipe_of);
                    s.spawn(move || -> Result<ExpandOut, ClosureError> {
                        let mut out = ExpandOut::default();
                        let (mut buf, mut cm, mut codes) = (vec![], vec![], vec![]);
                        let mut key = vec![0i32; arity];
                        for i in a..b {
                            let r = res[i];
                            if r == RECIPE {
                                codec.decode(table.code(i), &mut key);
                                let (si, ri) = recipe_of[&(i as u32)];
                                let mut sink = Sink::Bfs {
                                    out: &mut out,
                                    table,
                                    key_idx: i as u32,
                                };
                                expand_recipe(ctx, si, ri, &key, &mut buf, &mut codes, &mut sink);
                                continue;
                            }
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
        stop.resize(table.len(), STOP_NONE);
        start_mask.resize(table.len(), false);
        lo = hi;
    }
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
    identity
        .components
        .insert("start".into(), start_digest.clone());
    identity.components.insert(
        "inputs".into(),
        sha_hex(
            &inputs
                .iter()
                .map(|x| format!("{x:?}"))
                .collect::<Vec<_>>()
                .iter()
                .map(|x| x.as_str())
                .collect::<Vec<_>>(),
        ),
    );
    let unbound_tables: BTreeSet<String> = disp
        .tables()
        .iter()
        .filter(|t| !cfg.table_digests.contains_key(&t.name))
        .map(|t| t.name.clone())
        .collect();
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
        rule_digests: ctx.rule_digests.clone(),
        fact_tables: ctx.fact_tables.clone(),
        table_names: disp.tables().iter().map(|t| t.name.clone()).collect(),
        entry_table,
        fact_label: facts.label(),
        facts_exact: facts.exact(),
        facts_requested_by: facts.requested_by(),
        order_digest: dispatch_order_digest(disp),
        start_digest,
        lines: lines.clone(),
        identity,
        relaxed,
        hand_downs_mode: cfg.hand_downs,
        unbound_tables,
        disp_arity: arity,
        disp_chart: disp.chart().map(|c| c.to_vec()),
        disp_tables: disp
            .tables()
            .iter()
            .zip(&ids.table_dispatch)
            .map(|(t, d)| (t.name.clone(), d.clone()))
            .collect(),
        disp_priority: disp.priority().to_vec(),
        excl_names,
        incremental: cache.map(|_| inc_use),
        recipe_of,
        rset_names: ctx
            .rsets
            .iter()
            .map(|(_, x)| (x.name.clone(), x.recipe_digest.clone()))
            .collect(),
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

/// The current exclusions of `key` as (table name, record), only computed when needed for a cache comparison.
fn ctx_excl_names(ctx: &Ctx, key: &[i32]) -> Option<Vec<(String, usize)>> {
    let ps = ctx.excluded(key);
    if ps.is_empty() {
        return None;
    }
    let mut v: Vec<(String, usize)> = ps
        .iter()
        .map(|&p| {
            let r = &ctx.rule_names[p as usize];
            (r.table.clone(), r.record)
        })
        .collect();
    v.sort();
    v.dedup();
    Some(v)
}

/// The stopping position of a dispatch verdict (for the memo).
fn stop_of(ctx: &Ctx, d: &Dispatch) -> u32 {
    match d {
        Dispatch::Covered(r) => ctx.fast.position(*r),
        Dispatch::Blocked { at, .. } | Dispatch::Undecided { at, .. } => ctx.fast.position(*at),
        Dispatch::Uncovered => STOP_UNCOVERED,
        Dispatch::OutsideChart { .. } => STOP_NONE,
    }
}

/// The DISPOSITION of a dispatched key: declared terminal (checked first; the dispatch verdict is kept to detect a
/// terminal inside a rule's domain), covered (with Tier C regularity), credited, or open.
fn dispose(
    ctx: &Ctx,
    key: &[i32],
    d: Dispatch,
    rec: Option<(u32, u32)>,
) -> Result<KeyRes, ClosureError> {
    if let Some(kind) = ctx.terminal_kind(key) {
        let (would, dispatch) = match &d {
            _ if rec.is_some() => (None, TerminalDispatch::Recipe),
            Dispatch::Covered(r) => (Some(ctx.fast.position(*r)), TerminalDispatch::Covered),
            Dispatch::Blocked { at, .. } => {
                (Some(ctx.fast.position(*at)), TerminalDispatch::Blocked)
            }
            Dispatch::Undecided { at, .. } => {
                (Some(ctx.fast.position(*at)), TerminalDispatch::Undecided)
            }
            Dispatch::Uncovered => (None, TerminalDispatch::Uncovered),
            Dispatch::OutsideChart { .. } => (None, TerminalDispatch::OutsideChart),
        };
        return Ok(KeyRes::Terminal(TermInfo {
            kind,
            would,
            dispatch,
        }));
    }
    if let Some((si, ri)) = rec {
        let set = &ctx.rsets[si as usize].1;
        if set.relations.verdict(ri as usize) & VERDICT_CERTIFIED != VERDICT_CERTIFIED {
            return Ok(KeyRes::Open(OpenInfo {
                status: OpenStatus::RecipeUnverified,
                rule: None,
                detail: format!(
                    "relation set {}: verdict bits {}",
                    set.name,
                    set.relations.verdict(ri as usize)
                ),
                candidates: vec![si],
            }));
        }
        return Ok(KeyRes::Recipe(si, ri));
    }
    let tier_c = ctx.cfg.tier == Tier::C;
    Ok(match d {
        Dispatch::Covered(r) => {
            let p = ctx.fast.position(r);
            if !ctx.bodies.has(r.table, r.record) {
                return Ok(KeyRes::Open(OpenInfo {
                    status: OpenStatus::NoBody,
                    rule: Some(p),
                    detail: String::new(),
                    candidates: vec![],
                }));
            }
            if tier_c {
                match ctx.facts.regularity(&ctx.rid(p), key) {
                    RegFact::Pole => {
                        return Ok(KeyRes::Open(OpenInfo {
                            status: OpenStatus::Irregular,
                            rule: Some(p),
                            detail: "pole".into(),
                            candidates: vec![],
                        }));
                    }
                    RegFact::Unknown if ctx.relax_regularity => {
                        KeyRes::Pos(p, Some(RegFact::Regular))
                    }
                    f => KeyRes::Pos(p, Some(f)),
                }
            } else {
                KeyRes::Pos(p, None)
            }
        }
        Dispatch::Uncovered => {
            if let Some(pol) = &ctx.cfg.credits {
                match crate::symmetry::serve_with(key, pol, ctx)? {
                    Ok(c) => {
                        let reg = c
                            .undecided
                            .contains(&Refusal::RegularityUnknown)
                            .then_some(RegFact::Unknown);
                        return Ok(KeyRes::Credit(Box::new(c), reg));
                    }
                    Err(refusals) => {
                        let detail = format!("credit refused by {} element(s)", refusals.len());
                        return Ok(KeyRes::Open(OpenInfo {
                            status: OpenStatus::Uncovered,
                            rule: None,
                            detail,
                            candidates: ctx.recipe_candidates(key),
                        }));
                    }
                }
            }
            KeyRes::Open(OpenInfo {
                status: OpenStatus::Uncovered,
                rule: None,
                detail: String::new(),
                candidates: ctx.recipe_candidates(key),
            })
        }
        Dispatch::Blocked { at, reason } => KeyRes::Open(OpenInfo {
            status: OpenStatus::Blocked,
            rule: Some(ctx.fast.position(at)),
            detail: format!("{reason:?}"),
            candidates: vec![],
        }),
        Dispatch::Undecided { at, verdict } => KeyRes::Open(OpenInfo {
            status: OpenStatus::Undecided,
            rule: Some(ctx.fast.position(at)),
            detail: format!("{verdict:?}"),
            candidates: vec![],
        }),
        Dispatch::OutsideChart { slot } => KeyRes::Open(OpenInfo {
            status: OpenStatus::OutsideChart,
            rule: None,
            detail: format!("slot {slot}"),
            candidates: vec![],
        }),
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
        if r == RECIPE {
            ctx.codec.decode(cl.table.code(i as usize), key);
            let (si, ri) = cl.recipe_of[&i];
            let mut sink = Sink::Succ {
                table: &cl.table,
                succ: &mut succ,
            };
            expand_recipe(ctx, si, ri, key, buf, codes, &mut sink);
            return Ok(succ);
        }
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
            RECIPE => {
                let (si, ri) = self.recipe_of[&(i as u32)];
                Some(RuleName {
                    table: self.rset_names[si as usize].0.clone(),
                    record: ri as usize,
                    label: -1,
                })
            }
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
                recipe_candidates: o
                    .candidates
                    .iter()
                    .map(|&c| self.rset_names[c as usize].0.clone())
                    .collect(),
            })
            .collect()
    }
    /// Relation sets used (keys served per set, set order).
    pub fn used_relations(&self) -> Vec<RelationUse> {
        let mut v: Vec<(u64, u64)> = vec![(0, 0); self.rset_names.len()];
        for (&i, &(si, _)) in &self.recipe_of {
            v[si as usize].0 += 1;
            if self.start_mask[i as usize] {
                v[si as usize].1 += 1;
            }
        }
        v.into_iter()
            .enumerate()
            .filter(|(_, x)| x.0 > 0)
            .map(|(si, x)| RelationUse {
                set: self.rset_names[si].0.clone(),
                recipe_digest: self.rset_names[si].1.clone(),
                keys: x.0,
                keys_in_start: x.1,
            })
            .collect()
    }
    pub fn is_recipe_served(&self, i: usize) -> bool {
        self.res[i] == RECIPE
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
            .map(|(&i, t)| TerminalRecord {
                key: self.table.key(i as usize),
                in_start: self.start_mask[i as usize],
                kind: t.kind,
                would_dispatch: t.would.map(|p| self.rule_names[p as usize].clone()),
                dispatch: t.dispatch,
                in_rule_domain: t.in_rule_domain(),
            })
            .collect()
    }
    /// The configuration identity of this run (all inputs).
    pub fn config_identity(&self) -> &ConfigIdentity {
        &self.identity
    }
    /// How an incremental run used its cache (`None` for a run without a cache).
    pub fn incremental_use(&self) -> Option<&IncrementalUse> {
        self.incremental.as_ref()
    }
    /// The exit (hand-down) key sets with their completeness for THIS run's configuration (audit loop-A M3): complete
    /// iff the hand-down mode is `keys` and the run is not truncated.
    pub fn exit_sets(&self) -> ExitSets<'_> {
        ExitSets {
            complete: self.exits_complete(),
            config_digest: self.identity.digest(),
            sets: &self.hand_down_keys,
        }
    }
    fn exits_complete(&self) -> bool {
        self.hand_downs_mode == HandDowns::Keys && !self.truncated
    }
    pub fn is_terminal(&self, i: usize) -> bool {
        self.res[i] == TERMINAL
    }
    pub fn hand_down_sets(&self) -> &BTreeMap<u64, KeySet> {
        &self.hand_down_keys
    }
    /// Fact requests (sorted, de-duplicated): each carries the rule's content digest, the term's instantiated key and
    /// the configuration identity (without facts / start / inputs) of this run.
    pub fn fact_requests(&self) -> Vec<FactRequest> {
        let config = self.identity.digest_without(FACT_BINDING_SKIPS);
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
                    rhs: (r.term != u32::MAX).then(|| r.rhs.to_vec()),
                    reason: r.reason,
                    rule_digest: Some(self.rule_digests[r.pos as usize].clone()),
                    config: Some(config.clone()),
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
                OPEN | PENDING | TERMINAL | RECIPE => continue,
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
    /// The dispatch memo for [`closure_incremental`] (dispatch verdicts only, with the dispatch identity).
    pub fn dispatch_cache(&self) -> DispatchCache {
        DispatchCache {
            arity: self.disp_arity,
            bits: self.codec.bits(),
            chart: self.disp_chart.clone(),
            tables: self.disp_tables.clone(),
            priority: self.disp_priority.clone(),
            exclusions: self.excl_names.clone(),
            table: self.table.clone(),
            stop: self
                .stop
                .iter()
                .map(|&p| match p {
                    STOP_NONE => CachedStop::NotEvaluated,
                    STOP_UNCOVERED => CachedStop::Uncovered,
                    p => {
                        let r = &self.rule_names[p as usize];
                        CachedStop::At(self.entry_table[p as usize], r.record as u32)
                    }
                })
                .collect(),
            config: self.identity.clone(),
        }
    }

    fn terminal_violations(&self) -> u64 {
        self.terminals
            .values()
            .filter(|t| t.in_rule_domain())
            .count() as u64
    }

    /// Tables without a coefficient-file digest whose rules fire (Tier C: blocks `closed`).
    fn unbound_firing_tables(&self) -> Vec<String> {
        let fires = self.fires();
        let mut v: BTreeSet<String> = BTreeSet::new();
        for (p, f) in fires.iter().enumerate() {
            if f.0 > 0 {
                let t = &self.table_names[self.entry_table[p] as usize];
                if self.unbound_tables.contains(t) {
                    v.insert(t.clone());
                }
            }
        }
        v.into_iter().collect()
    }

    /// The verdict without the diagnostic stamp, and every reason it is not `closed`.
    fn verdict_parts(&self) -> (Verdict, Vec<String>) {
        let mut blocking = vec![];
        if self.violations > 0 {
            blocking.push(format!("descent_violations:{}", self.violations));
        }
        if self.pos_nonzero > 0 {
            blocking.push(format!("positive_numerator_nonzero:{}", self.pos_nonzero));
        }
        if self.unrepresentable > 0 {
            blocking.push(format!("unrepresentable:{}", self.unrepresentable));
        }
        if let Some(c) = self.cycles.filter(|&c| c > 0) {
            blocking.push(format!("cycles:{c}"));
        }
        let failed = !blocking.is_empty();
        if !self.open.is_empty() {
            blocking.push(format!("open_keys:{}", self.open.len()));
        }
        let tv = self.terminal_violations();
        if tv > 0 {
            blocking.push(format!("terminal_in_rule_domain:{tv}"));
        }
        if self.tier == Tier::K {
            blocking.push("tier_k_superset".into());
        }
        if self.undecided.total() > 0 {
            blocking.push(format!("undecided:{}", self.undecided.total()));
        }
        if self.truncated {
            blocking.push("truncated".into());
        }
        let mut provisional = false;
        if self.tier == Tier::C {
            if !self.facts_exact {
                blocking.push("facts_provisional".into());
                provisional = true;
            }
            if self.inputs.iter().any(|x| x.provisional_input()) {
                blocking.push("inputs_measured".into());
                provisional = true;
            }
            for t in self.unbound_firing_tables() {
                blocking.push(format!("table_coefficient_digest_missing:{t}"));
                provisional = true;
            }
        }
        let v = if failed {
            Verdict::Failed
        } else if !self.open.is_empty() {
            Verdict::Open
        } else if tv > 0 {
            Verdict::TerminalInRuleDomain
        } else if self.tier == Tier::K {
            Verdict::SupersetClean
        } else if self.undecided.total() > 0 || self.truncated {
            Verdict::Undecided
        } else if !provisional {
            Verdict::Closed
        } else {
            Verdict::ClosedProvisional
        };
        if !self.relaxed.is_empty() {
            blocking.push(format!("diagnostic:{}", self.relaxed.join(",")));
        }
        (v, blocking)
    }

    fn verdict(&self) -> Verdict {
        if !self.relaxed.is_empty() {
            Verdict::Diagnostic
        } else {
            self.verdict_parts().0
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
        let parts = self.verdict_parts();
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
                .filter(|t| t.kind == TerminalKind::Preferred)
                .count() as u64,
            terminals_other: self
                .terminals
                .values()
                .filter(|t| t.kind == TerminalKind::Other)
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
            terminal_in_rule_domain: self.terminal_violations(),
            diagnostic: !self.relaxed.is_empty(),
            relaxed_checks: self.relaxed.clone(),
            verdict_if_not_diagnostic: (!self.relaxed.is_empty()).then_some(parts.0),
            blocking: parts.1,
            config_digest: self.identity.digest(),
            exits_complete: self.exits_complete(),
            facts_requested_by: self.facts_requested_by.clone(),
            recipe_served: self.recipe_of.len() as u64,
            recipe_served_in_start: self
                .recipe_of
                .keys()
                .filter(|&&i| self.start_mask[i as usize])
                .count() as u64,
            used_relation_sets: self.used_relations().len() as u64,
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
        let mut config = self.identity.components.clone();
        config.insert("digest".into(), self.identity.digest());
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
            used_relations: self.used_relations(),
            timing: None,
        }
    }
}

/// Narrow view of the closure context for the symmetry module.
pub(crate) mod internal {
    use super::*;
    pub(crate) trait CtxView {
        /// The first-match dispatch WITH the rule-level key exclusions (the shipped table's dispatch).
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
        /// The tier of the run the credit is used in (there is no separate credit tier).
        fn tier(&self) -> Tier;
        /// Diagnostic relaxation: an Unknown regularity is not counted.
        fn relax_regularity(&self) -> bool;
        fn is_terminal(&self, key: &[i32]) -> bool;
        /// Whether a relation set serves `key` given its table dispatch `d` (then the table rule does not fire there).
        fn recipe_serves(&self, key: &[i32], d: &Dispatch) -> bool;
    }
    impl CtxView for Ctx<'_> {
        fn dispatch(&self, key: &[i32]) -> Dispatch {
            let (mut b, mut s) = (vec![], vec![]);
            self.fast
                .first_match_ex(key, 0, None, self.excluded(key), &mut b, &mut s)
                .0
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
        fn tier(&self) -> Tier {
            self.cfg.tier
        }
        fn relax_regularity(&self) -> bool {
            self.relax_regularity
        }
        fn is_terminal(&self, key: &[i32]) -> bool {
            self.terminal_kind(key).is_some()
        }
        fn recipe_serves(&self, key: &[i32], d: &Dispatch) -> bool {
            self.recipe_match(key, d).is_some()
        }
    }

    /// Context for the public standalone credit checker (outside a closure run): no exclusions, no terminals.
    pub(crate) struct StandaloneCtx<'a> {
        pub(crate) fast: FastDispatch<'a>,
        pub(crate) disp: &'a Dispatcher,
        pub(crate) bodies: &'a RuleBodies,
        pub(crate) cmp: &'a NodeComparator,
        pub(crate) sectors: Sectors,
        pub(crate) facts: &'a dyn FactSource,
        pub(crate) names: Vec<RuleName>,
        pub(crate) digests: Vec<String>,
        pub(crate) tier: Tier,
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
                digest: &self.digests[pos as usize],
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
        fn tier(&self) -> Tier {
            self.tier
        }
        fn relax_regularity(&self) -> bool {
            false
        }
        fn is_terminal(&self, _: &[i32]) -> bool {
            false
        }
        fn recipe_serves(&self, _: &[i32], _: &Dispatch) -> bool {
            false
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
