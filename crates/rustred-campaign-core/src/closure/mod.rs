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
mod credit;
pub mod exits;
pub mod facts;
pub mod fast;
pub mod keyset;
pub mod relations;
pub mod sha256;
pub mod store;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use crate::cover::{CoverError, Dispatch, Dispatcher, RuleRef};
use crate::order::NodeComparator;
use crate::symmetry::{CreditPolicy, Refusal};
use credit::{CreditC, Server};
use store::{ExitAcc, ReqStore};

pub use bodies::{
    BodyError, BodyLoadStats, MAX_ARITY, RhsSlot, RuleBodies, Sectors, TermClass, parse_rhs_pattern,
};
pub use facts::{
    CoefFact, FactError, FactKind, FactRequest, FactSource, FileFacts, NoFacts, RegFact,
    RequestReason, RuleId, write_fact_requests,
};
pub use fast::FastDispatch;
pub use keyset::{KeyCodec, KeySet, KeySetBuilder, KeySetError, KeyTable};
pub use relations::{
    DispatchSlot, RelationSet, RelationStore, RelationStoreBuilder, SolveResultsStats,
    VERDICT_CERTIFIED, VERDICT_DESCENDS, VERDICT_EXACT, VERDICT_REGULAR, read_solve_results,
};
pub use store::{ExitFile, SpillConfig};

pub const CLOSURE_SCHEMA: &str = "rustred.campaign.closure.v1";
/// Version of the closure semantics recorded in every configuration identity (bump on any change of a result).
pub const CLOSURE_SEMANTICS: &str = "closure-semantics.v4";

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
    /// Records WITHOUT witness loci (core `Blocked(WitnessNotCarried)`): `lift_with_regular_fact` (default) lets Tier C
    /// serve such a key iff the fact source holds the per-key exact lift fact (regular, in case, no exception branch)
    /// for exactly (rule content, key); Tier K always reports `block`. `Elided` records are never lifted.
    pub witness_absent: WitnessAbsent,
    /// Spill the large stores (fact requests, exit key sets) to files under a byte budget (re-audit F5, F9; closure.md
    /// 15). Not part of the identity: it never changes a result.
    pub spill: Option<SpillConfig>,
    /// The tables' presented frame (`node_order.slot_permutation`, new slot j carries old slot `p[j]`; identity =
    /// `0..arity`). When given it must equal the comparator's presentation (typed `Config` error otherwise; audit
    /// loop-A m7); recorded in the identity component `frame` (`unchecked` when absent: the caller's responsibility).
    pub table_presentation: Option<Vec<usize>>,
    /// The per-key lift (closure.md section 16; coordinator / user ruling 2026-11-14): `Proof` lets a record whose
    /// witness loci are ELIDED or ABSENT (core `Blocked(Elided | WitnessNotCarried)`) serve a key iff the fact source
    /// holds an exact per-key proof bound to exactly (rule content, key) ([`FactSource::per_key_proof`]), in BOTH
    /// tiers; such keys are recorded per key (`Closure::is_per_key_lifted`, summary `per_key_lifted`). The dispatcher
    /// stays strict (no witness check is turned off: never `diagnostic`). Without the proof the key stays open
    /// (`blocked`); Tier K follows the record's body all the same (superset of every Tier C run, re-audit F1b).
    /// `Off` (default): unchanged behaviour (identity unchanged).
    pub per_key_lift: PerKeyLift,
}

/// The per-key lift of records with elided / absent witness loci (closure.md section 16).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PerKeyLift {
    #[default]
    Off,
    /// Served iff an exact per-key proof is bound to (rule content, key).
    Proof,
}

/// Policy for records without witness loci (coordinator ruling 2026-11-14).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WitnessAbsent {
    Block,
    #[default]
    LiftWithRegularFact,
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
            witness_absent: WitnessAbsent::default(),
            spill: None,
            table_presentation: None,
            per_key_lift: PerKeyLift::Off,
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
    /// More reached keys than the u32 key index allows (checked before every insertion), or a request store limit.
    Capacity(String),
    /// Arity above the closure's slot-mask width [`MAX_ARITY`] (re-audit F4: refused, never a silent wrap).
    Arity {
        arity: usize,
        max: usize,
    },
    /// A spill file could not be written or read.
    Io(String),
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
    /// Dispatch position in the table (the producer's `record`).
    pub record: usize,
    pub label: i64,
    /// Position among the rule records of the source file (core `RecordMeta::source_index`; stable under skipped
    /// records). Rules are identified by content digest + source index, never by dispatch position.
    #[serde(default)]
    pub source_index: usize,
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

/// A symmetry credit used by the closure. `rule` names what serves the image: a table rule, or a relation set (table =
/// set name, record = its record, label -1) for a recipe-served image (R44).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditRecord {
    pub key: Vec<i32>,
    pub in_start: bool,
    pub element: usize,
    pub image: Vec<i32>,
    pub rule: RuleName,
    pub descent_checked: bool,
    /// Tier K only: the further structurally admissible elements whose expansions are followed as well (the superset
    /// follows the union, re-audit F1a). Empty in Tier C.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<CreditAlternative>,
}

/// A further element followed by a Tier K credit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreditAlternative {
    pub element: usize,
    pub image: Vec<i32>,
    pub rule: RuleName,
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
    /// Keys served through a witness-absent record lifted by its per-key exact fact, per table.
    pub witness_lifted: BTreeMap<String, u64>,
    /// Keys blocked by a witness-absent record (they require the per-key regularity fact; requested).
    pub requires_regularity_fact: u64,
    /// The effective witness-absent policy of this run (`block` in Tier K).
    pub witness_absent: WitnessAbsent,
    /// Keys served through an elided / witness-absent record by an exact per-key proof (`PerKeyLift::Proof`), per
    /// table (evidence level `per-key` at exactly those keys; closure.md section 16). Absent when empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub per_key_lifted: BTreeMap<String, u64>,
    /// `PerKeyLift::Proof`: keys blocked by an ELIDED record without its per-key proof (open). Absent when 0.
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub requires_per_key_proof: u64,
    /// Keys served by relation sets, and the number of sets used.
    pub recipe_served: u64,
    pub recipe_served_in_start: u64,
    pub used_relation_sets: u64,
    /// Tier K credits: top terms of a followed element that are NOT below the key (followed; the element is usable
    /// only if their coefficients vanish; re-audit F1a).
    #[serde(default)]
    pub credit_requires_zero: u64,
    /// Tier K: OPEN keys whose expansion is followed all the same (superset of every Tier C run; re-audit F1): a
    /// witness-absent key (its record's body; a Tier C run may lift it) or an uncovered key with credit candidates none
    /// of which passes the keys-only check (a Tier C run may credit it once coefficients are known to vanish).
    #[serde(default)]
    pub open_followed: u64,
    /// The dispatcher's weakened checks (witness check off, unchecked attestation, no chart): any entry makes the run
    /// `diagnostic` (re-audit F2).
    #[serde(default)]
    pub dispatch_not_strict: Vec<String>,
    /// The fact requests / exit key sets were spilled to files (`ClosureConfig::spill`).
    #[serde(default)]
    pub fact_requests_spilled: bool,
    #[serde(default)]
    pub exits_spilled: bool,
}

/// Use of one relation set (R48): keys it serves, and credited keys whose image it serves (R44).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationUse {
    pub set: String,
    pub recipe_digest: String,
    pub keys: u64,
    pub keys_in_start: u64,
    #[serde(default)]
    pub credit_images: u64,
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

/// Keys per work piece of a round (dispatch + expansion + merge): bounds the per-piece buffers (exit codes, requests,
/// dispositions) independently of the round size.
const PIECE_KEYS: usize = 1 << 14;
/// Exit-code words a piece buffers before an in-piece sort + dedup.
const PIECE_HD_WORDS: usize = 1 << 20;

impl OpenInfo {
    /// Tier K: what is followed although the key is open.
    fn followed(&self) -> Option<Served<'_>> {
        match (&self.follow, &self.follow_credit) {
            (Some(p), _) => Some(Served::Follow(*p)),
            (None, Some(c)) => Some(Served::Credit(c)),
            (None, None) => None,
        }
    }
}

#[derive(Clone, Debug)]
struct OpenInfo {
    status: OpenStatus,
    rule: Option<u32>,
    detail: String,
    candidates: Vec<u32>,
    /// Tier K: the record whose body is followed although the key is open (witness-absent; re-audit F1b).
    follow: Option<u32>,
    /// Tier K: credit candidates followed although the key is open (no element passes the keys-only check; F1a).
    follow_credit: Option<Box<CreditC>>,
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

/// The exit (hand-down) key sets of a run, tied to the run's configuration identity: in memory (`sets`) or, when the
/// run spilled them (`ClosureConfig::spill`), as key-set files (`files`; then `sets` is empty).
#[derive(Clone, Debug)]
pub struct ExitSets<'a> {
    /// True iff the sets are complete for this configuration (hand-down mode `keys`, not truncated).
    pub complete: bool,
    pub config_digest: String,
    pub sets: &'a BTreeMap<u64, KeySet>,
    pub files: &'a BTreeMap<u64, ExitFile>,
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
    credits: BTreeMap<u32, CreditC>,
    rounds: u32,
    truncated: bool,
    edges: u64,
    terms: TermStats,
    requires_zero: u64,
    credit_requires_zero: u64,
    undecided: Undecided,
    violations: u64,
    pos_nonzero: u64,
    unrepresentable: u64,
    descent_witnesses: Vec<(u32, u32, Witness)>,
    failure_witnesses: Vec<(u32, u32, Witness)>,
    requests: ReqStore,
    /// distinct fact requests (as `fact_requests()` would de-duplicate them), computed once at the end of the run
    n_requests: u64,
    hand_down_terms: BTreeMap<u64, u64>,
    hand_down_keys: BTreeMap<u64, KeySet>,
    exit_files: BTreeMap<u64, ExitFile>,
    /// the run's own spill directory, if it spilled
    spill_dir: Option<PathBuf>,
    cycles: Option<u64>,
    chain: Option<Chain>,
    /// a cycle was found although no followed edge was counted non-lower (the comparator is not a strict order)
    comparator_inconsistent: bool,
    /// the dispatcher's weakened checks (re-audit F2)
    not_strict: Vec<String>,
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
    lifted: BTreeSet<u32>,
    per_key_lifted: BTreeSet<u32>,
    witness_absent_keys: BTreeSet<u32>,
    witness_policy: WitnessAbsent,
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
            // what the dispatch and the reports rest on besides the domain (re-audit F7): witness status, witness and
            // target basis, source index; any change invalidates a recheck and the incremental reuse of the table
            let meta = match tab.meta.get(r) {
                Some(m) => format!(
                    "{:?}|{:?}|{:?}|{}",
                    m.witness, m.witness_basis, m.target_basis, m.source_index
                ),
                None => "no-meta".into(),
            };
            hd.update(&(meta.len() as u64).to_le_bytes());
            hd.update(meta.as_bytes());
            let mut hr = sha256::Sha256::new();
            hr.update(b"rustred.campaign.rule.v1\n");
            hr.update(src.as_bytes());
            hr.update(b"\n");
            let src_index = tab.meta.get(r).map(|m| m.source_index).unwrap_or(r);
            hr.update(&(src_index as u64).to_le_bytes());
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
            source_index: r.source_index,
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

/// Output of one work piece of a round.
struct ExpandOut {
    new_codes: Vec<u64>,
    edges: u64,
    terms: TermStats,
    requires_zero: u64,
    credit_requires_zero: u64,
    undecided: Undecided,
    violations: u64,
    pos_nonzero: u64,
    unrepresentable: u64,
    descent_w: Vec<(u32, u32, Witness)>,
    failure_w: Vec<(u32, u32, Witness)>,
    requests: ReqStore,
    req_error: Option<String>,
    hd_terms: BTreeMap<u64, u64>,
    hd_codes: BTreeMap<u64, Vec<u64>>,
    hd_words: usize,
    hd_limit: usize,
}

impl ExpandOut {
    fn new(codec: KeyCodec) -> ExpandOut {
        ExpandOut {
            new_codes: vec![],
            edges: 0,
            terms: TermStats::default(),
            requires_zero: 0,
            credit_requires_zero: 0,
            undecided: Undecided::default(),
            violations: 0,
            pos_nonzero: 0,
            unrepresentable: 0,
            descent_w: vec![],
            failure_w: vec![],
            requests: ReqStore::new(codec),
            req_error: None,
            hd_terms: BTreeMap::new(),
            hd_codes: BTreeMap::new(),
            hd_words: 0,
            hd_limit: PIECE_HD_WORDS,
        }
    }
    /// Buffer one exit code of child `m`; sort + dedup the piece's buffers when they exceed the limit (re-audit F9).
    fn hand_down(&mut self, m: u64, code: &[u64]) {
        let w = code.len();
        self.hd_codes.entry(m).or_default().extend_from_slice(code);
        self.hd_words += w;
        if self.hd_words > self.hd_limit {
            let mut n = 0;
            for v in self.hd_codes.values_mut() {
                keyset::sort_dedup(w, v);
                n += v.len();
            }
            self.hd_words = n;
            self.hd_limit = self.hd_limit.max(2 * n);
        }
    }
    fn request(
        &mut self,
        key: u32,
        pos: u32,
        term: Option<usize>,
        reason: RequestReason,
        image: bool,
        rhs: Option<&[i32]>,
    ) {
        if let Err(e) = self.requests.push(key, pos, term, reason, image, rhs) {
            self.req_error.get_or_insert(e);
        }
    }
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

/// How a reached key is expanded (shared by the BFS and the chain / cycle pass).
#[derive(Clone, Copy)]
enum Served<'a> {
    /// Covered (or lifted) by the rule at this dispatch position.
    Rule(u32),
    /// Served by a relation-set record.
    Recipe(u32, u32),
    /// Credited: every followed element's server at its image (Tier C: one; Tier K: the union).
    Credit(&'a CreditC),
    /// Tier K: open (witness-absent) but the record's body is followed (superset; re-audit F1b).
    Follow(u32),
}

/// The per-key results the expansion needs (borrowed from the run or from a finished closure).
struct ResView<'a> {
    res: &'a [u32],
    credits: &'a BTreeMap<u32, CreditC>,
    recipe_of: &'a BTreeMap<u32, (u32, u32)>,
    open: &'a BTreeMap<u32, OpenInfo>,
}

impl<'a> ResView<'a> {
    fn served(&self, i: u32) -> Option<Served<'a>> {
        match self.res[i as usize] {
            PENDING | TERMINAL => None,
            OPEN => self.open.get(&i).and_then(|o| o.followed()),
            RECIPE => {
                let (si, ri) = self.recipe_of[&i];
                Some(Served::Recipe(si, ri))
            }
            CREDITED => Some(Served::Credit(&self.credits[&i])),
            p => Some(Served::Rule(p)),
        }
    }
}

/// Expand `key` as `served` says: append the followed top-sector codes to `codes` (sorted and de-duplicated at the
/// end: the key's distinct edges); with `out`, account every term (the BFS), without it only the successors (the
/// chain / cycle pass).
#[allow(clippy::too_many_arguments)]
fn expand_served(
    ctx: &Ctx,
    served: Served,
    key: &[i32],
    buf: &mut Vec<i32>,
    cm: &mut Vec<u64>,
    codes: &mut Vec<u64>,
    mut out: Option<&mut ExpandOut>,
    key_idx: u32,
) -> Result<(), ClosureError> {
    codes.clear();
    match served {
        Served::Rule(p) | Served::Follow(p) => {
            expand_key(ctx, p, key, key, false, buf, cm, codes, out, key_idx)?
        }
        Served::Recipe(si, ri) => expand_recipe(ctx, si, ri, key, buf, codes, out, key_idx),
        Served::Credit(c) => {
            for (_, image, server) in c.followed() {
                match server {
                    Server::Rule(p) => expand_key(
                        ctx,
                        p,
                        image,
                        key,
                        true,
                        buf,
                        cm,
                        codes,
                        out.as_deref_mut(),
                        key_idx,
                    )?,
                    Server::Recipe(si, ri) => {
                        expand_recipe(ctx, si, ri, key, buf, codes, out.as_deref_mut(), key_idx)
                    }
                }
            }
        }
    }
    keyset::sort_dedup(ctx.codec.words(), codes);
    Ok(())
}

/// Expand a key served by a relation set: its RECORDED right-hand-side keys (exact at the key: no facts needed).
/// Top keys are edges (descent checked against `key`, the key itself or, for a credit, the ORIGINAL key); a removed
/// line is a hand-down; a positive numerator key is a failure (the recorded right-hand side omits vanishing terms).
#[allow(clippy::too_many_arguments)]
fn expand_recipe(
    ctx: &Ctx,
    si: u32,
    ri: u32,
    key: &[i32],
    buf: &mut Vec<i32>,
    codes: &mut Vec<u64>,
    mut out: Option<&mut ExpandOut>,
    key_idx: u32,
) {
    let set = &ctx.rsets[si as usize].1;
    let st = &set.relations;
    let w = ctx.codec.words();
    let sw = st.codec().words();
    buf.clear();
    buf.resize(ctx.arity, 0);
    let rname = || RuleName {
        table: set.name.clone(),
        record: ri as usize,
        label: -1,
        source_index: ri as usize,
    };
    let cap = ctx.cfg.witness_cap;
    let mut code = [0u64; keyset::MAX_WORDS];
    for (j, c) in st.rhs_codes(ri as usize).chunks_exact(sw).enumerate() {
        st.codec().decode(c, buf);
        let t: &[i32] = buf;
        let class = ctx.sectors.classify_masked(t, 0);
        let enc = ctx.codec.encode(t, &mut code);
        let Some(out) = out.as_deref_mut() else {
            if class == TermClass::Top && enc {
                codes.extend_from_slice(&code[..w]);
            }
            continue;
        };
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
                    (key_idx, j as u32, wit("recipe_positive_numerator")),
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
                            out.hand_down(dropped, &code[..w]);
                        } else {
                            out.unrepresentable += 1;
                            push_capped(
                                &mut out.failure_w,
                                (key_idx, j as u32, wit("unrepresentable_hand_down")),
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
                        (key_idx, j as u32, wit("descent_violation")),
                        cap,
                    );
                }
                if enc {
                    codes.extend_from_slice(&code[..w]);
                } else {
                    out.unrepresentable += 1;
                    push_capped(
                        &mut out.failure_w,
                        (key_idx, j as u32, wit("unrepresentable_key")),
                        cap,
                    );
                }
            }
        }
    }
}

/// Expand one rule at one key: `applied` = the key the rule is instantiated at (the image for a credit), `key` = the
/// key itself (every comparison is with it). Appends the followed top codes to `codes`.
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
    mut out: Option<&mut ExpandOut>,
    key_idx: u32,
) -> Result<(), ClosureError> {
    let rule = ctx.fast.rule(pos);
    buf.clear();
    cm.clear();
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
        let Some(out) = out.as_deref_mut() else {
            if fact == CoefFact::Zero || class != TermClass::Top {
                continue;
            }
            if ctx.codec.encode(t, &mut code) {
                codes.extend_from_slice(&code[..w]);
            }
            continue;
        };
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
                            key_idx,
                            j as u32,
                            witness(ctx, pos, key, j, t, "positive_numerator_nonzero"),
                        ),
                        cap,
                    );
                } else {
                    out.undecided.coef_unknown_positive_numerator += 1;
                    out.request(
                        key_idx,
                        pos,
                        Some(j),
                        RequestReason::PositiveNumerator,
                        image,
                        Some(t),
                    );
                }
            }
            TermClass::ZeroSector { .. } => {
                out.terms.zero_sector += 1;
            }
            TermClass::Subsector { dropped } => {
                out.terms.subsector += 1;
                match ctx.cfg.hand_downs {
                    HandDowns::Off => {}
                    HandDowns::Counts => *out.hd_terms.entry(dropped).or_default() += 1,
                    HandDowns::Keys => {
                        *out.hd_terms.entry(dropped).or_default() += 1;
                        if ctx.codec.encode(t, &mut code) {
                            out.hand_down(dropped, &code[..w]);
                        } else {
                            out.unrepresentable += 1;
                            push_capped(
                                &mut out.failure_w,
                                (
                                    key_idx,
                                    j as u32,
                                    witness(ctx, pos, key, j, t, "unrepresentable_hand_down"),
                                ),
                                cap,
                            );
                        }
                    }
                }
            }
            TermClass::Top => {
                out.terms.top += 1;
                if !ctx.cmp.lower(t, key) {
                    if image && !tier_c {
                        // Tier K credit (superset): the element is usable only if this coefficient vanishes
                        out.credit_requires_zero += 1;
                    } else if !tier_c || fact == CoefFact::NonZero {
                        out.violations += 1;
                        push_capped(
                            &mut out.descent_w,
                            (
                                key_idx,
                                j as u32,
                                witness(ctx, pos, key, j, t, "descent_violation"),
                            ),
                            cap,
                        );
                    } else {
                        out.undecided.coef_unknown_not_lower += 1;
                        out.request(
                            key_idx,
                            pos,
                            Some(j),
                            RequestReason::NotLower,
                            image,
                            Some(t),
                        );
                    }
                }
                if ctx.codec.encode(t, &mut code) {
                    codes.extend_from_slice(&code[..w]);
                } else {
                    out.unrepresentable += 1;
                    push_capped(
                        &mut out.failure_w,
                        (
                            key_idx,
                            j as u32,
                            witness(ctx, pos, key, j, t, "unrepresentable_key"),
                        ),
                        cap,
                    );
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

/// Split `lo..hi` into contiguous pieces of at most [`PIECE_KEYS`] keys, at least `threads` pieces when there are
/// enough keys (the result never depends on the split).
fn pieces(lo: usize, hi: usize, threads: usize) -> Vec<(usize, usize)> {
    let n = hi - lo;
    let t = threads.max(1).min(n.max(1));
    let c = n.div_ceil(t).clamp(1, PIECE_KEYS);
    (0..n.div_ceil(c))
        .map(|i| (lo + i * c, (lo + (i + 1) * c).min(hi)))
        .filter(|(a, b)| a < b)
        .collect()
}

/// Dispatch result of one key (internal).
enum KeyRes {
    Terminal(TermInfo),
    /// Served through a witness-absent record lifted by its exact per-key fact.
    Lifted(u32),
    /// Served through an elided / witness-absent record by an exact per-key proof (`PerKeyLift::Proof`).
    PerKey(u32),
    /// Blocked by a witness-absent record: open, with a lift request (Tier K: its body is followed).
    WitnessAbsent(OpenInfo),
    Recipe(u32, u32),
    Pos(u32, Option<RegFact>),
    Credit(Box<CreditC>),
    Open(OpenInfo),
}

impl KeyRes {
    fn served(&self) -> Option<Served<'_>> {
        match self {
            KeyRes::Terminal(_) => None,
            KeyRes::Lifted(p) | KeyRes::PerKey(p) | KeyRes::Pos(p, _) => Some(Served::Rule(*p)),
            KeyRes::WitnessAbsent(o) | KeyRes::Open(o) => o.followed(),
            KeyRes::Recipe(si, ri) => Some(Served::Recipe(*si, *ri)),
            KeyRes::Credit(c) => Some(Served::Credit(c)),
        }
    }
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

/// The effective witness-absent policy (Tier K: always `block`).
fn is_zero_u64(x: &u64) -> bool {
    *x == 0
}

fn witness_policy(cfg: &ClosureConfig) -> WitnessAbsent {
    if cfg.tier == Tier::K {
        WitnessAbsent::Block
    } else {
        cfg.witness_absent
    }
}

fn hand_downs_name(h: HandDowns) -> &'static str {
    match h {
        HandDowns::Off => "off",
        HandDowns::Counts => "counts",
        HandDowns::Keys => "keys",
    }
}

/// The dispatcher's weakened checks as the closure counts them (re-audit F2): no chart, a witness check turned off,
/// a witness or target resting on an unchecked attestation. Hand-built records (`Assurance::Caller`: the caller built
/// the domain, loci included) are not a weakened check; they are visible in the identity's `dispatcher_summary`.
fn not_strict_reasons(disp: &Dispatcher) -> Vec<String> {
    let s = disp.summary();
    let mut v = vec![];
    if !s.chart_checked {
        v.push("chart_unchecked".to_string());
    }
    let list = |kind: &str, names: Vec<&str>| -> Option<String> {
        match names.len() {
            0 => None,
            1..=4 => Some(format!("{kind}:{}", names.join("+"))),
            n => Some(format!("{kind}:{n}_tables")),
        }
    };
    let off: Vec<&str> = s
        .tables
        .iter()
        .filter(|t| t.witness_check_off > 0)
        .map(|t| t.name.as_str())
        .collect();
    let att: Vec<&str> = s
        .tables
        .iter()
        .filter(|t| t.witness_attested_unchecked > 0 || t.target_attested_unchecked > 0)
        .map(|t| t.name.as_str())
        .collect();
    v.extend(list("witness_check_off", off));
    v.extend(list("attested_unchecked", att));
    v
}

fn io_err(e: std::io::Error) -> ClosureError {
    ClosureError::Io(e.to_string())
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
    if arity > MAX_ARITY {
        return Err(ClosureError::Arity {
            arity,
            max: MAX_ARITY,
        });
    }
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
    let frame = match &cfg.table_presentation {
        None => "unchecked (the caller's responsibility, closure.md 11.7)".to_string(),
        Some(p) => {
            if p.len() != arity {
                return Err(ClosureError::KeyArity {
                    expected: arity,
                    got: p.len(),
                });
            }
            if p.as_slice() != cmp.presentation().new_to_old() {
                return Err(ClosureError::Config(format!(
                    "the comparator's presentation {:?} differs from the tables' frame {p:?} (audit loop-A m7)",
                    cmp.presentation().new_to_old()
                )));
            }
            format!("checked:{p:?}")
        }
    };
    let not_strict = not_strict_reasons(disp);
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
        let p = fast.position(RuleRef::bare(ti, e.record, e.label));
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
    // credits rest only on the identity-test lane's verified group (directing session 2026-11-14): a hand-built group
    // is a relaxed check
    if cfg.credits.as_ref().is_some_and(|p| !p.is_verified()) {
        relaxed.push("credits_group_unverified".into());
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
        c.insert("frame".into(), frame.clone());
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
                    &format!("{:?}", p.group()),
                    &format!("{:?}", p.pure_numerator_maps()),
                    &format!("{:?}", p.basis()),
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
        c.insert(
            "witness_absent".into(),
            format!("{:?}", witness_policy(cfg)),
        );
        if cfg.per_key_lift != PerKeyLift::Off {
            c.insert("per_key_lift".into(), format!("{:?}", cfg.per_key_lift));
        }
        // what the dispatcher's credits rest on (witness check on / off, attestations checked / unchecked, chart)
        c.insert("dispatcher_summary".into(), format!("{:?}", disp.summary()));
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
        relax_regularity: cfg.diagnostic.allow_unknown_regularity || !cfg.require_regularity,
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
    let mut credits: BTreeMap<u32, CreditC>;
    let mut terminals: BTreeMap<u32, TermInfo>;
    let mut recipe_of: BTreeMap<u32, (u32, u32)>;
    let mut lifted: BTreeSet<u32>;
    let mut per_key_lifted: BTreeSet<u32>;
    let mut witness_absent_keys: BTreeSet<u32>;
    let mut acc = ExpandOut::new(codec);
    let mut start_mask: Vec<bool>;
    let mut prev_rounds = 0u32;
    let mut inputs: Vec<InputLabel> = vec![];
    // spill (re-audit F5, F9): the run's own directory, created only when a spill is configured
    let spill_dir = match &cfg.spill {
        Some(sp) => Some(store::make_run_dir(&sp.dir).map_err(io_err)?),
        None => None,
    };
    let budget = cfg.spill.as_ref().map(|s| s.budget_bytes as usize);
    let mut exits = ExitAcc::new(codec, spill_dir.clone().zip(budget));
    match resume {
        None => {
            table = KeyTable::new(codec, start.len().max(1) * 2);
            res = vec![];
            stop = vec![];
            open = BTreeMap::new();
            credits = BTreeMap::new();
            terminals = BTreeMap::new();
            recipe_of = BTreeMap::new();
            lifted = BTreeSet::new();
            per_key_lifted = BTreeSet::new();
            witness_absent_keys = BTreeSet::new();
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
            lifted = p.lifted.clone();
            per_key_lifted = p.per_key_lifted.clone();
            witness_absent_keys = p.witness_absent_keys.clone();
            exits
                .seed(&p.hand_down_keys, &p.exit_files)
                .map_err(io_err)?;
            start_mask = p.start_mask.clone();
            prev_rounds = p.rounds;
            inputs = p.inputs.clone();
            acc.edges = p.edges;
            acc.terms = p.terms.clone();
            acc.requires_zero = p.requires_zero;
            acc.credit_requires_zero = p.credit_requires_zero;
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
    if let (Some(d), Some(b)) = (&spill_dir, budget) {
        acc.requests.set_spill(d.join("fact-requests.bin"), b);
    }
    inputs.extend(cfg.inputs.iter().cloned());
    inputs.sort();
    inputs.dedup();
    let index_cap = (RECIPE as usize).saturating_sub(8);
    let mut lo = table.len();
    for i in 0..start.len() {
        if table.len() + 1 >= index_cap {
            return Err(ClosureError::Capacity(format!(
                "{} reached keys: u32 key index exhausted (audit loop-A m6)",
                table.len() + 1
            )));
        }
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
    // a recheck counts ITS OWN rounds against max_rounds (re-audit F6b): the keys it adds are exactly those reachable
    // only from the extra keys, at the same depth as in the closure of the union, so the same keys are truncated
    let mut rounds = 0u32;
    let mut truncated = false;
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
                        candidates: vec![],
                        follow: None,
                        follow_credit: None,
                    },
                );
            }
            break;
        }
        rounds += 1;
        // ---- per piece: dispatch (always, also at declared terminals), disposition, expansion; merged in key order
        let parts = pieces(lo, hi, cfg.threads);
        type PieceOut = (Vec<(KeyRes, u32, bool)>, ExpandOut);
        let mut new_codes: Vec<u64> = vec![];
        for group in parts.chunks(cfg.threads.max(1)) {
            let results: Vec<Result<PieceOut, ClosureError>> = std::thread::scope(|s| {
                let hs: Vec<_> = group
                    .iter()
                    .map(|&(a, b)| {
                        let (ctx, table, reusable, view) = (&ctx, &table, &reusable, &view);
                        s.spawn(move || -> Result<PieceOut, ClosureError> {
                            let mut out = ExpandOut::new(codec);
                            let (mut buf, mut st) = (vec![], vec![]);
                            let (mut ebuf, mut cm, mut codes) = (vec![], vec![], vec![]);
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
                                                            ctx.fast.position(RuleRef::bare(
                                                                j, r as usize, 0,
                                                            )),
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
                                let kr = dispose(ctx, &key, d, rec)?;
                                if let Some(sv) = kr.served() {
                                    expand_served(
                                        ctx,
                                        sv,
                                        &key,
                                        &mut ebuf,
                                        &mut cm,
                                        &mut codes,
                                        Some(&mut out),
                                        i as u32,
                                    )?;
                                    out.edges += (codes.len() / w) as u64;
                                    for c in codes.chunks_exact(w) {
                                        if table.get(c).is_none() {
                                            out.new_codes.extend_from_slice(c);
                                        }
                                    }
                                }
                                v.push((kr, stp, used_cache));
                            }
                            keyset::sort_dedup(w, &mut out.new_codes);
                            for c in out.hd_codes.values_mut() {
                                keyset::sort_dedup(w, c);
                            }
                            Ok((v, out))
                        })
                    })
                    .collect();
                hs.into_iter()
                    .map(|h| h.join().expect("closure worker"))
                    .collect()
            });
            let mut i = group[0].0;
            for part in results {
                let (disp_out, mut o) = part?;
                for (kr, st, used_cache) in disp_out {
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
                                acc.request(
                                    i as u32,
                                    p,
                                    None,
                                    RequestReason::Regularity,
                                    false,
                                    None,
                                );
                            }
                        }
                        KeyRes::Credit(c) => {
                            res[i] = CREDITED;
                            if let (true, Server::Rule(p)) = (c.regularity_unknown, c.server) {
                                acc.undecided.regularity_unknown += 1;
                                acc.request(
                                    i as u32,
                                    p,
                                    None,
                                    RequestReason::Regularity,
                                    true,
                                    None,
                                );
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
                        KeyRes::Lifted(p) => {
                            res[i] = p;
                            lifted.insert(i as u32);
                        }
                        KeyRes::PerKey(p) => {
                            res[i] = p;
                            per_key_lifted.insert(i as u32);
                        }
                        KeyRes::WitnessAbsent(o) => {
                            let p = o.rule.expect("blocking record");
                            res[i] = OPEN;
                            open.insert(i as u32, o);
                            witness_absent_keys.insert(i as u32);
                            acc.request(i as u32, p, None, RequestReason::WitnessLift, false, None);
                        }
                        KeyRes::Recipe(si, ri) => {
                            res[i] = RECIPE;
                            recipe_of.insert(i as u32, (si, ri));
                        }
                    }
                    i += 1;
                }
                // merge the piece's expansion (key order)
                new_codes.extend_from_slice(&o.new_codes);
                acc.edges += o.edges;
                acc.terms.add(&o.terms);
                acc.requires_zero += o.requires_zero;
                acc.credit_requires_zero += o.credit_requires_zero;
                acc.undecided.coef_unknown_positive_numerator +=
                    o.undecided.coef_unknown_positive_numerator;
                acc.undecided.coef_unknown_not_lower += o.undecided.coef_unknown_not_lower;
                acc.violations += o.violations;
                acc.pos_nonzero += o.pos_nonzero;
                acc.unrepresentable += o.unrepresentable;
                merge_capped(&mut acc.descent_w, o.descent_w, cfg.witness_cap);
                merge_capped(&mut acc.failure_w, o.failure_w, cfg.witness_cap);
                if let Some(e) = o.req_error.take() {
                    return Err(ClosureError::Capacity(e));
                }
                acc.requests.append(&mut o.requests).map_err(io_err)?;
                for (k, v) in o.hd_terms {
                    *acc.hd_terms.entry(k).or_default() += v;
                }
                for (k, v) in o.hd_codes {
                    exits.add(k, &v).map_err(io_err)?;
                }
            }
            if let Some(e) = acc.req_error.take() {
                return Err(ClosureError::Capacity(e));
            }
            keyset::sort_dedup(w, &mut new_codes);
        }
        keyset::sort_dedup(w, &mut new_codes);
        // capacity BEFORE any insertion (audit loop-A m6)
        if table.len() + new_codes.len() / w >= index_cap {
            return Err(ClosureError::Capacity(format!(
                "{} reached keys: u32 key index exhausted (audit loop-A m6)",
                table.len() + new_codes.len() / w
            )));
        }
        for c in new_codes.chunks_exact(w) {
            table.insert(c);
        }
        res.resize(table.len(), PENDING);
        stop.resize(table.len(), STOP_NONE);
        start_mask.resize(table.len(), false);
        lo = hi;
    }
    // rounds of a recheck: max(previous, continuation), an upper bound of the union's (closure.md 11.4, B-m1)
    let rounds = rounds.max(prev_rounds);
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
    let exit_peak = exits.peak_bytes();
    let (hand_down_keys, exit_files) = exits.finish().map_err(io_err)?;
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
        credit_requires_zero: acc.credit_requires_zero,
        undecided: acc.undecided,
        violations: acc.violations,
        pos_nonzero: acc.pos_nonzero,
        unrepresentable: acc.unrepresentable,
        descent_witnesses: acc.descent_w,
        failure_witnesses: acc.failure_w,
        requests: acc.requests,
        n_requests: 0,
        hand_down_terms: acc.hd_terms,
        hand_down_keys,
        exit_files,
        spill_dir,
        cycles: None,
        chain: None,
        comparator_inconsistent: false,
        not_strict,
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
        lifted,
        per_key_lifted,
        witness_absent_keys,
        witness_policy: witness_policy(cfg),
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
    cl.n_requests = cl.count_requests().map_err(io_err)?;
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
        + exit_peak
        + cl.requests.memory_bytes();
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
                follow: None,
                follow_credit: None,
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
                    follow: None,
                    follow_credit: None,
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
                            follow: None,
                            follow_credit: None,
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
                let (detail, follow_credit) = match credit::serve_credit(ctx, key, pol)? {
                    credit::CreditOutcome::Credited(c) => return Ok(KeyRes::Credit(Box::new(c))),
                    credit::CreditOutcome::OpenFollowed(c, n) => (
                        format!(
                            "credit refused by {n} element(s); {} candidate(s) followed (Tier K)",
                            1 + c.alternatives.len()
                        ),
                        Some(Box::new(c)),
                    ),
                    credit::CreditOutcome::Refused(n) => {
                        (format!("credit refused by {n} element(s)"), None)
                    }
                };
                return Ok(KeyRes::Open(OpenInfo {
                    status: OpenStatus::Uncovered,
                    rule: None,
                    detail,
                    candidates: ctx.recipe_candidates(key),
                    follow: None,
                    follow_credit,
                }));
            }
            KeyRes::Open(OpenInfo {
                status: OpenStatus::Uncovered,
                rule: None,
                detail: String::new(),
                candidates: ctx.recipe_candidates(key),
                follow: None,
                follow_credit: None,
            })
        }
        Dispatch::Blocked { at, reason }
            if ctx.cfg.per_key_lift == PerKeyLift::Proof
                && (reason == crate::domain::Verdict::WitnessNotCarried
                    || reason == crate::domain::Verdict::Elided)
                && ctx.bodies.has(at.table, at.record)
                && ctx
                    .facts
                    .per_key_proof(&ctx.rid(ctx.fast.position(at)), key) =>
        {
            KeyRes::PerKey(ctx.fast.position(at))
        }
        Dispatch::Blocked { at, reason }
            if reason == crate::domain::Verdict::Elided
                && ctx.cfg.per_key_lift == PerKeyLift::Proof =>
        {
            // open without its per-key proof; Tier K follows the body (a Tier C run with the proof serves it)
            let p = ctx.fast.position(at);
            let follow = (!tier_c && ctx.bodies.has(at.table, at.record)).then_some(p);
            KeyRes::Open(OpenInfo {
                status: OpenStatus::Blocked,
                rule: Some(p),
                detail: format!("{reason:?}: requires the per-key proof"),
                candidates: vec![],
                follow,
                follow_credit: None,
            })
        }
        Dispatch::Blocked { at, reason } if reason == crate::domain::Verdict::WitnessNotCarried => {
            let p = ctx.fast.position(at);
            if witness_policy(ctx.cfg) == WitnessAbsent::LiftWithRegularFact
                && ctx.bodies.has(at.table, at.record)
                && ctx.facts.witness_lift(&ctx.rid(p), key) == RegFact::Regular
            {
                return Ok(KeyRes::Lifted(p));
            }
            // Tier K (superset of every Tier C run, re-audit F1b): the record's body is followed although the key
            // stays open (a Tier C run may lift it with the exact per-key fact)
            let follow = (!tier_c && ctx.bodies.has(at.table, at.record)).then_some(p);
            KeyRes::WitnessAbsent(OpenInfo {
                status: OpenStatus::Blocked,
                rule: Some(p),
                detail: format!("{reason:?}: requires the per-key regularity fact"),
                candidates: vec![],
                follow,
                follow_credit: None,
            })
        }
        Dispatch::Blocked { at, reason } => KeyRes::Open(OpenInfo {
            status: OpenStatus::Blocked,
            rule: Some(ctx.fast.position(at)),
            detail: format!("{reason:?}"),
            candidates: vec![],
            follow: None,
            follow_credit: None,
        }),
        Dispatch::Undecided { at, verdict } => KeyRes::Open(OpenInfo {
            status: OpenStatus::Undecided,
            rule: Some(ctx.fast.position(at)),
            detail: format!("{verdict:?}"),
            candidates: vec![],
            follow: None,
            follow_credit: None,
        }),
        Dispatch::OutsideChart { slot } => KeyRes::Open(OpenInfo {
            status: OpenStatus::OutsideChart,
            rule: None,
            detail: format!("slot {slot}"),
            candidates: vec![],
            follow: None,
            follow_credit: None,
        }),
    })
}

/// Cycle check and longest chain over the actual edges (successors recomputed, no stored edges; re-audit F3, F6a):
/// an iterative Tarjan pass. `cycles` = the number of non-trivial strongly connected components (a key on a cycle,
/// a self-edge included): independent of the reach order. The longest chain is computed only when the graph is
/// acyclic (a longest path of a DAG); ties broken by the smaller key (code order), so the chain does not depend on the
/// reach order either. Memory: 9 bytes per key + the depth-first stack. The VERDICT never uses this pass: a cycle
/// needs a followed non-lower edge (the comparator is a total order), which the run has already counted as a
/// violation, an undecided coefficient or a Tier K credit term requiring zero; a cycle without any such edge
/// (`comparator_inconsistent`) fails the run.
fn chain_pass(ctx: &Ctx, cl: &mut Closure) -> Result<(), ClosureError> {
    let n = cl.table.len();
    // per key: state (0 new, 1 on the Tarjan stack, 2 done); a = index / height, b = lowlink / next on the chain
    let mut state = vec![0u8; n];
    let mut a = vec![0u32; n];
    let mut b = vec![u32::MAX; n];
    let mut counter = 0u32;
    let mut sccs = 0u64;
    let (mut buf, mut cm, mut codes) = (vec![], vec![], vec![]);
    let mut key = vec![0i32; ctx.arity];
    let view = ResView {
        res: &cl.res,
        credits: &cl.credits,
        recipe_of: &cl.recipe_of,
        open: &cl.open,
    };
    let table = &cl.table;
    let mut succ_of = |i: u32| -> Result<(Vec<u32>, bool), ClosureError> {
        let mut succ = vec![];
        if let Some(sv) = view.served(i) {
            ctx.codec.decode(table.code(i as usize), &mut key);
            expand_served(ctx, sv, &key, &mut buf, &mut cm, &mut codes, None, i)?;
            for c in codes.chunks_exact(ctx.codec.words()) {
                if let Some(j) = table.get(c) {
                    succ.push(j);
                }
            }
        }
        let self_loop = succ.contains(&i);
        Ok((succ, self_loop))
    };
    // the Tarjan stack (keys of open components) and the depth-first frames (key, successors, cursor, self loop)
    let mut tstack: Vec<u32> = vec![];
    let mut frames: Vec<(u32, Vec<u32>, usize, bool)> = vec![];
    for root in 0..n as u32 {
        if state[root as usize] != 0 {
            continue;
        }
        let (s0, l0) = succ_of(root)?;
        state[root as usize] = 1;
        a[root as usize] = counter;
        b[root as usize] = counter;
        counter += 1;
        tstack.push(root);
        frames.push((root, s0, 0, l0));
        while let Some(top) = frames.last_mut() {
            let v = top.0;
            if top.2 < top.1.len() {
                let u = top.1[top.2];
                top.2 += 1;
                match state[u as usize] {
                    0 => {
                        let (su, lu) = succ_of(u)?;
                        state[u as usize] = 1;
                        a[u as usize] = counter;
                        b[u as usize] = counter;
                        counter += 1;
                        tstack.push(u);
                        frames.push((u, su, 0, lu));
                    }
                    1 => {
                        // on the stack: lowlink(v) = min(lowlink(v), index(u))
                        let iu = a[u as usize];
                        if iu < b[v as usize] {
                            b[v as usize] = iu;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            let (v, succ, _, self_loop) = frames.pop().expect("frame");
            let (iv, lv) = (a[v as usize], b[v as usize]);
            if let Some(parent) = frames.last() {
                let p = parent.0 as usize;
                if lv < b[p] {
                    b[p] = lv;
                }
            }
            if lv != iv {
                continue; // v belongs to a component rooted further up
            }
            // v roots a component: pop it
            let mut members = 0usize;
            while let Some(x) = tstack.pop() {
                state[x as usize] = 2;
                members += 1;
                if x == v {
                    break;
                }
            }
            if members > 1 || self_loop {
                sccs += 1;
                a[v as usize] = 0;
                b[v as usize] = u32::MAX;
                continue;
            }
            // a trivial component: every successor is done; height and next (ties: the smaller key)
            let mut best = 0u32;
            let mut bn = u32::MAX;
            for &u in &succ {
                let h = a[u as usize] + 1;
                if h > best
                    || (h == best
                        && bn != u32::MAX
                        && table.code(u as usize) < table.code(bn as usize))
                {
                    best = h;
                    bn = u;
                }
            }
            a[v as usize] = best;
            b[v as usize] = bn;
        }
    }
    cl.cycles = Some(sccs);
    if sccs > 0 {
        cl.chain = None;
        let counted = cl.violations + cl.undecided.coef_unknown_not_lower + cl.credit_requires_zero;
        cl.comparator_inconsistent = counted == 0;
        return Ok(());
    }
    if n > 0 {
        let h = &a;
        let next = &b;
        let mut top = 0usize;
        for i in 1..n {
            if h[i] > h[top] || (h[i] == h[top] && cl.table.code(i) < cl.table.code(top)) {
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
                    source_index: ri as usize,
                })
            }
            CREDITED => Some(self.server_name(self.credits[&(i as u32)].server)),
            p => Some(self.rule_names[p as usize].clone()),
        }
    }
    /// The name of what serves a credit's image: a table rule, or a relation-set record (label -1).
    fn server_name(&self, s: Server) -> RuleName {
        match s {
            Server::Rule(p) => self.rule_names[p as usize].clone(),
            Server::Recipe(si, ri) => RuleName {
                table: self.rset_names[si as usize].0.clone(),
                record: ri as usize,
                label: -1,
                source_index: ri as usize,
            },
        }
    }
    pub fn is_open(&self, i: usize) -> bool {
        self.res[i] == OPEN
    }
    pub fn is_credited(&self, i: usize) -> bool {
        self.res[i] == CREDITED
    }
    /// Whether key `i` is served by an elided / witness-absent record through its exact per-key proof
    /// (`PerKeyLift::Proof`; evidence level `per-key` at this key).
    pub fn is_per_key_lifted(&self, i: usize) -> bool {
        self.per_key_lifted.contains(&(i as u32))
    }
    /// Whether key `i` is served by a witness-absent record lifted by its per-key regularity fact (Tier C).
    pub fn is_witness_lifted(&self, i: usize) -> bool {
        self.lifted.contains(&(i as u32))
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
        let mut v: Vec<(u64, u64, u64)> = vec![(0, 0, 0); self.rset_names.len()];
        for (&i, &(si, _)) in &self.recipe_of {
            v[si as usize].0 += 1;
            if self.start_mask[i as usize] {
                v[si as usize].1 += 1;
            }
        }
        for c in self.credits.values() {
            for (_, _, s) in c.followed() {
                if let Server::Recipe(si, _) = s {
                    v[si as usize].2 += 1;
                }
            }
        }
        v.into_iter()
            .enumerate()
            .filter(|(_, x)| x.0 > 0 || x.2 > 0)
            .map(|(si, x)| RelationUse {
                set: self.rset_names[si].0.clone(),
                recipe_digest: self.rset_names[si].1.clone(),
                keys: x.0,
                keys_in_start: x.1,
                credit_images: x.2,
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
                rule: self.server_name(c.server),
                descent_checked: true,
                alternatives: c
                    .alternatives
                    .iter()
                    .map(|(e, img, s)| CreditAlternative {
                        element: *e,
                        image: img.clone(),
                        rule: self.server_name(*s),
                    })
                    .collect(),
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
            files: &self.exit_files,
        }
    }
    /// Spilled exit key sets (one key-set file per child sector; empty when they are in memory).
    pub fn exit_files(&self) -> &BTreeMap<u64, ExitFile> {
        &self.exit_files
    }
    /// The run's own spill directory (`None`: nothing configured).
    pub fn spill_dir(&self) -> Option<&std::path::Path> {
        self.spill_dir.as_deref()
    }
    /// Per child sector: (distinct exit keys, key-set digest), from memory or from the spilled files.
    pub fn exit_counts(&self) -> BTreeMap<u64, (u64, String)> {
        let mut m: BTreeMap<u64, (u64, String)> = self
            .hand_down_keys
            .iter()
            .map(|(&k, s)| (k, (s.len() as u64, s.digest())))
            .collect();
        for (&k, f) in &self.exit_files {
            m.insert(k, (f.count, f.digest.clone()));
        }
        m
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
    /// the configuration identity (without facts / start / inputs) of this run. Materialises every request: for a run
    /// with millions of requests use [`Closure::write_fact_requests_jsonl`] (streaming) and
    /// [`ClosureSummary::fact_requests`] (the count). Panics if a spill file became unreadable
    /// ([`Closure::try_fact_requests`] returns the error).
    pub fn fact_requests(&self) -> Vec<FactRequest> {
        self.try_fact_requests()
            .expect("fact requests: spill file unreadable")
    }
    /// As [`Closure::fact_requests`], with the spill-file error.
    pub fn try_fact_requests(&self) -> Result<Vec<FactRequest>, ClosureError> {
        let config = self.identity.digest_without(FACT_BINDING_SKIPS);
        let mut v = vec![];
        self.requests
            .for_each(|r| v.push(self.fact_request(&r, &config)))
            .map_err(io_err)?;
        v.sort();
        v.dedup();
        Ok(v)
    }
    fn fact_request(&self, r: &store::ReqItem, config: &str) -> FactRequest {
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
            kind: if matches!(
                r.reason,
                RequestReason::Regularity | RequestReason::WitnessLift
            ) {
                FactKind::Regularity
            } else {
                FactKind::Coefficient
            },
            term: r.term.map(|t| t as usize),
            rhs: r.rhs.map(|x| x.to_vec()),
            reason: r.reason,
            rule_digest: Some(self.rule_digests[r.pos as usize].clone()),
            config: Some(config.to_string()),
        }
    }
    /// Fact identity per dispatch position: positions with the same producer address (table path, record, label) and
    /// rule content digest give identical requests.
    fn fact_ids(&self) -> Vec<u32> {
        let mut ids: HashMap<(&str, usize, i64, &str), u32> = HashMap::new();
        (0..self.rule_names.len())
            .map(|p| {
                let r = &self.rule_names[p];
                let k = (
                    self.fact_tables[p].as_str(),
                    r.record,
                    r.label,
                    self.rule_digests[p].as_str(),
                );
                let n = ids.len() as u32;
                *ids.entry(k).or_insert(n)
            })
            .collect()
    }
    /// The image requests (distinct, production order) and, among them, those equal to a request of a reached key
    /// (which the non-image stream already carries): (requests, probe (fid, key index, term, reason) -> index).
    #[allow(clippy::type_complexity)]
    fn image_requests(
        &self,
        fid: &[u32],
    ) -> std::io::Result<(
        Vec<(u32, Vec<i32>, u32, RequestReason, u32, u32)>,
        HashMap<(u32, u32, u32, RequestReason), usize>,
    )> {
        let mut seen: HashSet<(u32, Vec<i32>, u32, RequestReason)> = HashSet::new();
        let mut img = vec![];
        self.requests.for_each(|r| {
            if r.image {
                let applied = self.credits[&r.key].image.clone();
                let t = r.term.unwrap_or(u32::MAX);
                let id = (fid[r.pos as usize], applied.clone(), t, r.reason);
                if seen.insert(id) {
                    img.push((fid[r.pos as usize], applied, t, r.reason, r.key, r.pos));
                }
            }
        })?;
        let mut probe = HashMap::new();
        for (i, (f, applied, t, rs, _, _)) in img.iter().enumerate() {
            if let Some(ix) = self.table.get_key(applied) {
                probe.insert((*f, ix, *t, *rs), i);
            }
        }
        Ok((img, probe))
    }
    /// The number of distinct fact requests, without materialising them (re-audit F5): requests of reached keys are
    /// distinct by construction (one disposition and one expansion per key); only image requests (credits) can repeat
    /// or coincide with a request of a reached key.
    fn count_requests(&self) -> std::io::Result<u64> {
        let fid = self.fact_ids();
        let mut non_image = 0u64;
        self.requests.for_each(|r| non_image += (!r.image) as u64)?;
        let (img, probe) = self.image_requests(&fid)?;
        if probe.is_empty() {
            return Ok(non_image + img.len() as u64);
        }
        let mut hit = vec![false; img.len()];
        self.requests.for_each(|r| {
            if !r.image
                && let Some(&i) = probe.get(&(
                    fid[r.pos as usize],
                    r.key,
                    r.term.unwrap_or(u32::MAX),
                    r.reason,
                ))
            {
                hit[i] = true;
            }
        })?;
        Ok(non_image + hit.iter().filter(|&&h| !h).count() as u64)
    }
    /// Stream the fact requests as JSONL (schema `rustred.campaign.fact-request.v1`, the lines of
    /// [`write_fact_requests`]) in CLOSURE order (requests of reached keys in key-index order per round, then the image
    /// requests not already written), each distinct request once; memory: the image requests only (re-audit F5).
    /// Returns the number of lines (= [`ClosureSummary::fact_requests`]).
    pub fn write_fact_requests_jsonl(&self, mut w: impl Write) -> Result<u64, ClosureError> {
        let config = self.identity.digest_without(FACT_BINDING_SKIPS);
        let fid = self.fact_ids();
        let (img, probe) = self.image_requests(&fid).map_err(io_err)?;
        let mut hit = vec![false; img.len()];
        let mut n = 0u64;
        let mut err: Option<std::io::Error> = None;
        self.requests
            .for_each(|r| {
                if r.image || err.is_some() {
                    return;
                }
                if let Some(&i) = probe.get(&(
                    fid[r.pos as usize],
                    r.key,
                    r.term.unwrap_or(u32::MAX),
                    r.reason,
                )) {
                    hit[i] = true;
                }
                let line = serde_json::to_string(&self.fact_request(&r, &config))
                    .expect("serialise a fact request");
                if let Err(e) = writeln!(w, "{line}") {
                    err = Some(e);
                }
                n += 1;
            })
            .map_err(io_err)?;
        if let Some(e) = err {
            return Err(io_err(e));
        }
        // the remaining image requests, from their first credited key (production order)
        let mut written: HashSet<(u32, Vec<i32>, u32, RequestReason)> = HashSet::new();
        self.requests
            .for_each(|r| {
                if !r.image || err.is_some() {
                    return;
                }
                let applied = self.credits[&r.key].image.clone();
                let t = r.term.unwrap_or(u32::MAX);
                let i = img
                    .iter()
                    .position(|x| {
                        x.0 == fid[r.pos as usize] && x.1 == applied && x.2 == t && x.3 == r.reason
                    })
                    .expect("image request collected");
                if hit[i] || !written.insert((img[i].0, applied, t, r.reason)) {
                    return;
                }
                let line = serde_json::to_string(&self.fact_request(&r, &config))
                    .expect("serialise a fact request");
                if let Err(e) = writeln!(w, "{line}") {
                    err = Some(e);
                }
                n += 1;
            })
            .map_err(io_err)?;
        if let Some(e) = err {
            return Err(io_err(e));
        }
        w.flush().map_err(io_err)?;
        Ok(n)
    }
    /// Per dispatch position: (fires on all keys, fires on start keys). A rule fires at a key it serves, at a credited
    /// key whose followed image it serves (Tier K: every followed element), and (Tier K) at a witness-absent key whose
    /// body is followed: the used set of a Tier K run contains that of every Tier C run (re-audit F1, R20).
    fn fires(&self) -> Vec<(u64, u64)> {
        let mut f = vec![(0u64, 0u64); self.rule_names.len()];
        let mut ps: Vec<u32> = vec![];
        for (i, &r) in self.res.iter().enumerate() {
            ps.clear();
            match r {
                PENDING | TERMINAL | RECIPE => {}
                OPEN => match self.open.get(&(i as u32)).and_then(|o| o.followed()) {
                    Some(Served::Follow(p)) => ps.push(p),
                    Some(Served::Credit(c)) => {
                        for (_, _, s) in c.followed() {
                            if let Server::Rule(p) = s {
                                ps.push(p);
                            }
                        }
                        ps.sort_unstable();
                        ps.dedup();
                    }
                    _ => {}
                },
                CREDITED => {
                    for (_, _, s) in self.credits[&(i as u32)].followed() {
                        if let Server::Rule(p) = s {
                            ps.push(p);
                        }
                    }
                    ps.sort_unstable();
                    ps.dedup();
                }
                p => ps.push(p),
            }
            for &p in &ps {
                f[p as usize].0 += 1;
                if self.start_mask[i] {
                    f[p as usize].1 += 1;
                }
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
        // a cycle always contains a followed non-lower edge, counted above or as undecided / credit_requires_zero, so
        // `cycles` never decides the verdict (re-audit F3); a cycle WITHOUT any such edge means the comparator is not a
        // strict order: fail
        if self.comparator_inconsistent {
            blocking.push(format!(
                "comparator_inconsistent:cycles:{}",
                self.cycles.unwrap_or(0)
            ));
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
        for r in &self.not_strict {
            blocking.push(format!("dispatch_not_strict:{r}"));
        }
        if !self.relaxed_checks().is_empty() {
            blocking.push(format!("diagnostic:{}", self.relaxed_checks().join(",")));
        }
        (v, blocking)
    }

    /// The relaxed checks: the configured relaxations and the dispatcher's weakened checks (re-audit F2).
    fn relaxed_checks(&self) -> Vec<String> {
        let mut v = self.relaxed.clone();
        v.extend(
            self.not_strict
                .iter()
                .map(|r| format!("dispatch_not_strict:{r}")),
        );
        v
    }

    fn verdict(&self) -> Verdict {
        if !self.relaxed_checks().is_empty() {
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
            // the serde (snake_case) name, as `OpenKey.status` serialises (re-audit N1)
            let name = serde_json::to_value(o.status)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_default();
            *by_status.entry(name).or_default() += 1;
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
        let exit_counts = self.exit_counts();
        let hand_downs = self
            .hand_down_terms
            .iter()
            .map(|(&m, &t)| (m, [t, exit_counts.get(&m).map(|x| x.0).unwrap_or(0)]))
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
            fact_requests: self.n_requests,
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
            diagnostic: !self.relaxed_checks().is_empty(),
            relaxed_checks: self.relaxed_checks(),
            verdict_if_not_diagnostic: (!self.relaxed_checks().is_empty()).then_some(parts.0),
            blocking: parts.1,
            config_digest: self.identity.digest(),
            exits_complete: self.exits_complete(),
            facts_requested_by: self.facts_requested_by.clone(),
            witness_lifted: {
                let mut m: BTreeMap<String, u64> = BTreeMap::new();
                for &i in &self.lifted {
                    let p = self.res[i as usize];
                    *m.entry(self.table_names[self.entry_table[p as usize] as usize].clone())
                        .or_default() += 1;
                }
                m
            },
            requires_regularity_fact: self.witness_absent_keys.len() as u64,
            witness_absent: self.witness_policy,
            per_key_lifted: {
                let mut m: BTreeMap<String, u64> = BTreeMap::new();
                for &i in &self.per_key_lifted {
                    let p = self.res[i as usize];
                    *m.entry(self.table_names[self.entry_table[p as usize] as usize].clone())
                        .or_default() += 1;
                }
                m
            },
            requires_per_key_proof: self
                .open
                .values()
                .filter(|o| {
                    o.status == OpenStatus::Blocked
                        && o.detail.ends_with("requires the per-key proof")
                })
                .count() as u64,
            recipe_served: self.recipe_of.len() as u64,
            recipe_served_in_start: self
                .recipe_of
                .keys()
                .filter(|&&i| self.start_mask[i as usize])
                .count() as u64,
            used_relation_sets: self.used_relations().len() as u64,
            credit_requires_zero: self.credit_requires_zero,
            open_followed: self
                .open
                .values()
                .filter(|o| o.followed().is_some())
                .count() as u64,
            dispatch_not_strict: self.not_strict.clone(),
            fact_requests_spilled: self.requests.is_spilled(),
            exits_spilled: !self.exit_files.is_empty(),
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
                .exit_counts()
                .into_iter()
                .map(|(m, (_, d))| (m, d))
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
        /// IMAGE LIFT (lane import3, closure.md 17): whether the record `at` that the strict dispatch reports blocked at
        /// the credit IMAGE for `reason` (elided / witness loci not carried) may serve the image, by the same evidence
        /// that lifts it at a key under the per-key lift: an exact per-key proof bound to (record, image) under
        /// `PerKeyLift::Proof` (a regularity fact alone never lifts an image: the re-audit's decision is kept). Returns
        /// `Some(true)` when lifted by a per-key proof (the proof implies regularity; `Some(false)` is reserved), `None`
        /// when not lifted. Default: never (the standalone checker).
        fn image_lift(
            &self,
            _at: RuleRef,
            _reason: crate::domain::Verdict,
            _image: &[i32],
        ) -> Option<bool> {
            None
        }
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
        fn image_lift(
            &self,
            at: RuleRef,
            reason: crate::domain::Verdict,
            image: &[i32],
        ) -> Option<bool> {
            use crate::domain::Verdict;
            if !matches!(reason, Verdict::WitnessNotCarried | Verdict::Elided)
                || !self.bodies.has(at.table, at.record)
            {
                return None;
            }
            // only an exact per-key PROOF lifts an image (a regularity fact never does: re-audit decision, test
            // audit_closure2_findings ok_credit_direct_comparison_and_no_chaining (iv))
            let rid = self.rid(self.fast.position(at));
            (self.cfg.per_key_lift == PerKeyLift::Proof && self.facts.per_key_proof(&rid, image))
                .then_some(true)
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
