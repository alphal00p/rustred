//! The configuration of the close-from-material source (`rustred.campaign.closeloop.v1`, interfaces/closeloop.md 2).
//! Everything node-specific (names, paths, orders, batch sizes, budgets) is DATA here (rule L8); unknown fields are
//! refused (a misspelt budget must not silently disable it).

use rustred_campaign_contract::fake::RecipeQuirk;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CONFIG_SCHEMA: &str = "rustred.campaign.closeloop.v1";
/// The role of the campaign-spec input that names the configuration file.
pub const CONFIG_ROLE: &str = "closeloop";

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CloseLoopConfig {
    pub schema: String,
    pub nodes: Vec<NodeCfg>,
}

/// What the start set of the import spec stands for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Target {
    /// The start set is the spec's box (R41 P-1): the node is closed as BOXED.
    #[default]
    Boxed,
    /// The start set is a full-coverage start set (FULL closure).
    Full,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandCfg {
    pub path: PathBuf,
    /// An `import::HandFormat` (column map of the hand statement).
    pub format: serde_json::Value,
    /// lane closeloop4 (audit-closeloop M1 / m7): the reference's sha256 (verified when given); in any case its digest
    /// is recorded in the ledger at first use (input `closeloop:<n>:pins`) and every later read is checked against it.
    #[serde(default)]
    pub sha256: Option<String>,
}

/// Work-counter budgets of one node (never a clock).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budgets {
    #[serde(default = "d_max_rounds")]
    pub max_rounds: u32,
    #[serde(default = "d_max_solve_keys")]
    pub max_solve_keys: u64,
    #[serde(default = "d_max_solve_jobs")]
    pub max_solve_jobs: u64,
    /// Attempts (jobs) of one solve item without a per-key decision before it goes to attention.
    #[serde(default = "d_max_job_attempts")]
    pub max_job_attempts: u32,
    /// Proof + fact jobs (rr_proof) the node may issue (lane closeloop2).
    #[serde(default = "d_max_solve_jobs")]
    pub max_proof_jobs: u64,
}

impl Default for Budgets {
    fn default() -> Self {
        Budgets {
            max_rounds: d_max_rounds(),
            max_solve_keys: d_max_solve_keys(),
            max_solve_jobs: d_max_solve_jobs(),
            max_job_attempts: d_max_job_attempts(),
            max_proof_jobs: d_max_solve_jobs(),
        }
    }
}

fn d_max_rounds() -> u32 {
    50
}
fn d_max_solve_keys() -> u64 {
    1_000_000
}
fn d_max_solve_jobs() -> u64 {
    100_000
}
fn d_max_job_attempts() -> u32 {
    2
}

/// TESTS ONLY: the fake child answers the batch from an oracle file (closeloop.md 2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeSolve {
    /// JSONL `{"source": <recipe source id>, "key": [..], "verdict": "SOLVED"|.., "rhs": [[..], ..]}`.
    pub oracle: PathBuf,
    #[serde(default)]
    pub ms: u64,
    /// Contract quirks the fake child emulates (a lying child).
    #[serde(default)]
    pub quirks: Vec<RecipeQuirk>,
    /// Program key of the fake child in `spec.programs` (default `rr_fake_child`).
    #[serde(default = "d_fake_program")]
    pub program: String,
}

fn d_fake_program() -> String {
    "rr_fake_child".into()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SolveCfg {
    /// Key of `spec.programs` (the engine with the solve-recipe job mode).
    pub program: String,
    pub family: PathBuf,
    /// The node order: a name or a NodeOrder JSON object (passed to `--order` verbatim / as compact JSON).
    pub order: serde_json::Value,
    #[serde(default = "d_keys_per_job")]
    pub max_keys_per_job: usize,
    #[serde(default = "d_one")]
    pub threads: u32,
    /// lane closeloop3 (closeloop.md 16.4): the per-key DETERMINISTIC work cap (`--max-trace-rows`; data, no default):
    /// a key over it ends `BUDGET_WORK` (work-decided) and goes to the NEXT recipe step of its escalation.
    #[serde(default)]
    pub max_trace_rows: Option<u64>,
    /// lane closeloop3 (closeloop.md 16.4): the per-key WALL cap (`--key-budget-s`), a safety net (default 1,800 s;
    /// `null` = none): a key stopped by it ends `BUDGET_WALL`, recorded TIME-DECIDED, and goes to the next recipe step.
    #[serde(default = "d_key_budget_s")]
    pub key_budget_s: Option<u64>,
    #[serde(default = "d_cap")]
    pub cap_bytes: u64,
    #[serde(default = "d_peak")]
    pub expected_peak_bytes: u64,
    #[serde(default = "d_heartbeat")]
    pub heartbeat_s: u32,
    #[serde(default = "d_grace")]
    pub stop_grace_s: u32,
    /// The determinism id the answers must carry (answers of another id are refused); None = the answers' own.
    #[serde(default)]
    pub determinism_id: Option<String>,
    /// The relation rule in force (coordinator's ruling: `m1_first` wanted): `m1_first` requires every answer to
    /// carry `relation_rule: m1_first` (engine v1.2); `first_admissible` (default) = the comparator's first hit
    /// (engine v1.1 has no field: then labelled so). Recorded in every batch file, answers line and result.
    #[serde(default = "d_relation_rule")]
    pub relation_rule: String,
    #[serde(default)]
    pub fake: Option<FakeSolve>,
}

fn d_relation_rule() -> String {
    // lane closeloop2 (closeloop.md 13.5): the coordinator's ruling, and the v1.2 recipe default
    "m1_first".into()
}

fn d_keys_per_job() -> usize {
    16
}
fn d_key_budget_s() -> Option<u64> {
    Some(1800)
}
fn d_one() -> u32 {
    1
}
fn d_cap() -> u64 {
    2 << 30
}
fn d_peak() -> u64 {
    1 << 30
}
fn d_heartbeat() -> u32 {
    10
}
fn d_grace() -> u32 {
    30
}

impl SolveCfg {
    /// The `--order` argument.
    pub fn order_arg(&self) -> String {
        match &self.order {
            serde_json::Value::String(s) => s.clone(),
            v => v.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeCfg {
    pub node: String,
    pub import_spec: PathBuf,
    #[serde(default)]
    pub import_base: Option<PathBuf>,
    #[serde(default)]
    pub target: Target,
    #[serde(default)]
    pub hand: Option<HandCfg>,
    #[serde(default = "d_threads")]
    pub check_threads: usize,
    /// R49: appended as the LAST recipe source of the import spec (faces first met while closing).
    #[serde(default)]
    pub default_recipe: Option<PathBuf>,
    /// Overrides the import spec's `required_evidence` (`imported` | `replay` | `per-key`).
    #[serde(default)]
    pub required_evidence: Option<String>,
    #[serde(default)]
    pub budgets: Budgets,
    pub solve: SolveCfg,
    /// lane closeloop2 (closeloop.md 13.2): close at the exact tier (Tier C, strict, rows spilled) once the facts of
    /// every served rule key are in (they are asked from `proof` jobs first).
    #[serde(default = "d_yes")]
    pub exact_tier: bool,
    /// lane closeloop2 (closeloop.md 13.1): per-key proof / fact jobs (rr_proof as a legacy child); None = none asked
    /// (then our relations stay below per-key and the exact tier cannot get its facts: reported, never assumed).
    #[serde(default)]
    pub proof: Option<ProofCfg>,
    /// lane closeloop3 (closeloop.md 16.9): the id of the BOX CLAUSE this run's start set stands for (data; None =
    /// `box`). An added clause of a node is run as an incremental extension: its own start set, the earlier runs'
    /// facts / proofs as evidence (nothing done is redone), the earlier clauses' verdicts in `prior_clauses`.
    #[serde(default)]
    pub box_clause: Option<String>,
    #[serde(default)]
    pub prior_clauses: Vec<PriorClause>,
    /// lane closeloop4 (closeloop.md 18): the standing second route of the exact tier's zero test (ON by default).
    #[serde(default)]
    pub second_route: SecondRouteCfg,
    /// lane closeloop4 (closeloop.md 19): which terminals are terminals (default: only the declared ones).
    #[serde(default)]
    pub terminals: TerminalsCfg,
    /// lane closeloop4 (audit-closeloop M3): what a reference (hand statement) mismatch does to a closed node.
    #[serde(default)]
    pub reference: ReferenceCfg,
    /// lane t266run (interfaces/evidence-store.md, closeloop.md 17): big-node mode (persistent table index, disk-backed
    /// evidence store, streamed keys-only pass); None = as before.
    #[serde(default)]
    pub scale: Option<ScaleCfg>,
    /// lane t266run (closeloop.md 17): the campaign's OWN escalation for keys nothing serves (symmetry relation at the
    /// key in-process, then per-key point recipes of a step sequence), and the finite-terminal rule that turns the keys
    /// the escalation cannot serve into terminal proposals; None = as before (only the spec's recipe sources).
    #[serde(default)]
    pub escalation: Option<EscalationCfg>,
    /// lane t266run (closeloop.md 17.4): the box as CLAUSES (data), e.g. a proved box and an added layer; the import
    /// spec's start must be their union; the result gives a verdict per clause and attributes every key to the first
    /// clause whose own closure contains it. None = one box.
    #[serde(default)]
    pub clauses: Option<Vec<ClauseCfg>>,
}

/// One clause of the box: an id and an import start spec (`{"path", "format", ..}`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClauseCfg {
    pub id: String,
    pub start: serde_json::Value,
}

/// The escalation of uncovered keys (DATA; lane t266run). Step names are the import's recipe-step grammar
/// (interfaces/import.md 14: `d<k>_c<centre>_f<on|off>[_s..][_m..]`), evaluated at each key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EscalationCfg {
    /// The exact symmetry relation at the key (verified group, expanded rows; termmin.md 2) before any recipe.
    #[serde(default = "d_yes")]
    pub symmetry_at_key: bool,
    /// The verified group (an import GroupSpec); None = derived from the frame table header's family.
    #[serde(default)]
    pub group: Option<serde_json::Value>,
    /// Bound of one expanded symmetry row.
    #[serde(default = "d_sym_terms")]
    pub symmetry_max_terms: usize,
    /// Recipe steps in escalation order (default [`d_steps`]).
    #[serde(default = "d_steps")]
    pub steps: Vec<String>,
    /// Slot priority of the step centres (`first` / `last` / `num`): lines and numerators in this order; None = slot
    /// order.
    #[serde(default)]
    pub coordinate_priority: Option<Vec<usize>>,
    /// A step whose explicit seed set at the key is larger is skipped (recorded).
    #[serde(default)]
    pub max_seeds: Option<u64>,
    /// The spec's finite-terminal rule: keys the escalation cannot serve become terminal PROPOSALS; they are accepted
    /// (declared terminals of the node) when there are at most `max_terminals` of them, else the node ends open.
    #[serde(default)]
    pub terminals: Option<TerminalRule>,
}

/// The user's "any finite terminal set" rule as data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalRule {
    /// Terminal proposals ACCEPTED without an operator (pre-approved keys, e.g. a list the operator decided earlier).
    #[serde(default)]
    pub approved: Vec<Vec<i32>>,
    /// More proposals than this means the escalation is not working: the node stops with a typed reason instead of
    /// piling up terminals.
    #[serde(default = "d_max_proposals")]
    pub max_proposals: usize,
    /// Accept up to this many proposals WITHOUT an operator decision (tests / unattended toy runs); None = every proposal
    /// not in `approved` waits for the operator (the run ends `open: awaiting operator decision`, exit 8).
    #[serde(default)]
    pub max_terminals: Option<usize>,
}

fn d_max_proposals() -> usize {
    8
}

fn d_sym_terms() -> usize {
    4096
}
/// The default escalation (after the exact symmetry relation at the key, in-process): depth-2 shells of the checker's
/// shell family (`_wdown`: S0, S1, Sm1, S1m1, S2) centred one step toward the floor on the last off-floor line, on a
/// NUMERATOR slot, on the first line and on every off-floor slot (coord-ec1-seed-centre.md v2), then without the
/// floor, depth 3 last (each solve under the per-key budget). Cheapest first (measured prior on a neighbouring node:
/// 78 % of unserved keys need only symmetry, 22 % a depth-2 recipe, depth 3 rare). All M1-first. Data overrides it.
fn d_steps() -> Vec<String> {
    [
        "d2_clast_fon_wdown_mM1C",
        "d2_cnum_fon_wdown_mM1C",
        "d2_cfirst_fon_wdown_mM1C",
        "d2_cunion_fon_wdown_mM1C",
        "d2_cnum_foff_wdown_mM1C",
        "d2_cfirst_foff_wdown_mM1C",
        "d3_cfirst_foff_wdown_mM1C",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Big-node settings (DATA; lane t266run). Results never depend on them, only memory and time do.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScaleCfg {
    /// Persistent table-index cache directory (import::index; a rerun of an unchanged table does not re-read it);
    /// None = CDIR/closeloop/<node>/index.
    #[serde(default)]
    pub index_dir: Option<PathBuf>,
    /// Evidence-store cache directory; None = CDIR/closeloop/<node>/store.
    #[serde(default)]
    pub store_dir: Option<PathBuf>,
    /// Sort budget of a store build (MiB).
    #[serde(default = "d_store_budget")]
    pub store_budget_mib: u64,
    /// Parse threads of a store build.
    #[serde(default = "d_store_threads")]
    pub store_threads: usize,
    /// The node's own fact / proof answer files go into stores of this many files each (full chunks are reused).
    #[serde(default = "d_store_chunk")]
    pub store_chunk_files: usize,
    /// Write the keys-only pass's rows to a file (only exceptional rows in memory) instead of keeping every row.
    #[serde(default = "d_yes")]
    pub stream_keys_only: bool,
    /// Run the keys-only pass before the exact tier (it lists every served rule key without a fact); false = the exact
    /// tier at once, its own fact / proof requests drive the jobs (when the imported evidence covers most keys).
    #[serde(default = "d_yes")]
    pub keys_only_first: bool,
    /// Refuse a spec evidence entry (facts / proofs / solved) that is a directory or carries no sha256 (audit-closeloop
    /// M1: an unpinned evidence file with one altered line changes the verdict without refusal).
    #[serde(default = "d_yes")]
    pub require_pinned_evidence: bool,
}

fn d_store_budget() -> u64 {
    192
}
fn d_store_threads() -> usize {
    4
}
fn d_store_chunk() -> usize {
    64
}

/// lane closeloop4 (closeloop.md 18): every zero-coefficient fact that changes the exact closure (a DECISIVE zero) is
/// confirmed by the other route (rr_proof instantiate <-> exact evaluation); records with elided loci get BOTH routes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecondRouteCfg {
    /// Confirm every decisive zero by the second route (a disagreement blocks the node).
    #[serde(default = "d_yes")]
    pub decisive_zeros: bool,
    /// Records with elided witness loci: their exact-evaluation facts are also asked from rr_proof
    /// (`--elided-loci facts`) and compared over every term (basis `exact_eval + rr_proof_elided`).
    #[serde(default = "d_yes")]
    pub elided: bool,
    /// lane zeros2 (closeloop.md 23): the second route of EVERY zero term the decided exact closure uses (positive
    /// numerators, non-decisive top terms, hand-downs, zero sectors): `structural` (DEFAULT: one proof per (record,
    /// term, locus) by `rr_coef_facts`, the exact per-key evaluation where no proof covers the key), `per_key` (the
    /// per-key evaluation only), `off` (closeloop4's behaviour: decisive / elided pairs only). A disagreement blocks.
    /// ABSENT (None) = `structural` (the DEFAULT, implementer decision 2026-11-15 after the real re-runs); a
    /// zero to confirm without `proof.exact_eval` blocks (`all_zeros_unavailable`): only an explicit `off` accepts a
    /// zero on one route.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub all_zeros: Option<AllZeros>,
    /// (record, key) pairs of one zeros job (records of one table).
    #[serde(default = "d_zeros_keys")]
    pub zeros_keys_per_job: usize,
    /// Asked terms of one zeros job.
    #[serde(default = "d_zeros_terms")]
    pub zeros_terms_per_job: usize,
}

/// lane zeros2: how every zero term is confirmed (see [`SecondRouteCfg::all_zeros`]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AllZeros {
    #[default]
    Structural,
    PerKey,
    Off,
}

impl AllZeros {
    pub fn word(self) -> &'static str {
        match self {
            AllZeros::Structural => "structural",
            AllZeros::PerKey => "per_key",
            AllZeros::Off => "off",
        }
    }
}

fn d_zeros_keys() -> usize {
    4096
}
fn d_zeros_terms() -> usize {
    200_000
}

impl Default for SecondRouteCfg {
    fn default() -> Self {
        SecondRouteCfg {
            decisive_zeros: true,
            elided: true,
            all_zeros: None,
            zeros_keys_per_job: d_zeros_keys(),
            zeros_terms_per_job: d_zeros_terms(),
        }
    }
}

/// lane closeloop4 (closeloop.md 19): an uncovered key that nothing more can serve is a terminal PROPOSAL.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TerminalAccept {
    /// Only the terminals the import spec / imported statement declares are terminals; proposals wait for an operator
    /// decision (`rr_campaign control CDIR resolve-terminals`): the run stops with exit 8 until then.
    #[default]
    DeclaredOnly,
    /// Proposals are accepted automatically when there are at most `max_auto` of them (explicit choice only); more
    /// wait for the operator as under `declared_only`.
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalsCfg {
    #[serde(default)]
    pub accept: TerminalAccept,
    #[serde(default = "d_max_auto")]
    pub max_auto: usize,
}

impl Default for TerminalsCfg {
    fn default() -> Self {
        TerminalsCfg {
            accept: TerminalAccept::DeclaredOnly,
            max_auto: d_max_auto(),
        }
    }
}

/// lane closeloop4 (audit-closeloop M3): `mismatch`: `block` (default: a key of the reference that ours lacks and that
/// the recorded decisive zeros do not explain keeps the node open) | `report` (listed only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReferenceCfg {
    #[serde(default = "d_block")]
    pub mismatch: String,
}

impl Default for ReferenceCfg {
    fn default() -> Self {
        ReferenceCfg {
            mismatch: d_block(),
        }
    }
}

fn d_block() -> String {
    "block".into()
}

fn d_max_auto() -> usize {
    8
}

/// An earlier box clause of the node, decided by another run (its final result record).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PriorClause {
    pub id: String,
    pub result: PathBuf,
    /// lane closeloop4 (audit-closeloop m7): the prior result's sha256 (verified when given); recorded in the ledger at
    /// first use either way (pins).
    #[serde(default)]
    pub sha256: Option<String>,
}

fn d_yes() -> bool {
    true
}

/// The proof jobs of a node (DATA; closeloop.md 13.7).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofCfg {
    /// Key of `spec.programs` (rr_proof; MANIFEST-pinned binary, see closeloop.md 13.1 for which build).
    pub program: String,
    /// `engine` (rows from `family`, engine build) | `cert` (rows from `cert`, std build).
    #[serde(default = "d_rows")]
    pub rows: String,
    #[serde(default)]
    pub family: Option<PathBuf>,
    #[serde(default)]
    pub cert: Option<PathBuf>,
    /// The chart's symmetry group record (one-term symmetry rows of recipes with `symmetry_rows`).
    #[serde(default)]
    pub symmetry_group: Option<PathBuf>,
    /// Our relations are stated modulo the proved-zero sectors (the verb drops them): prove them so too.
    #[serde(default = "d_yes")]
    pub drop_zero_sectors_relations: bool,
    /// Keys of one span job (one record).
    #[serde(default = "d_span_keys")]
    pub max_keys_per_job: usize,
    /// Keys of one facts job (records of one table).
    #[serde(default = "d_fact_keys")]
    pub max_fact_keys_per_job: usize,
    /// lane t266run: instantiated TERMS of one facts job (keys x the record's term count; rr_proof instantiate holds
    /// its whole output in memory and writes every term): None = no term cap.
    #[serde(default)]
    pub max_fact_terms_per_job: Option<u64>,
    #[serde(default = "d_one")]
    pub threads: u32,
    #[serde(default = "d_cap")]
    pub cap_bytes: u64,
    #[serde(default = "d_peak")]
    pub expected_peak_bytes: u64,
    /// Wall cap of one rr_proof job (a legacy child writes no EVENTS: the supervisor's silence timeout is the cap).
    #[serde(default = "d_proof_wall")]
    pub wall_s: u32,
    #[serde(default = "d_yes")]
    pub import_requests: bool,
    #[serde(default = "d_yes")]
    pub relations: bool,
    #[serde(default = "d_yes")]
    pub facts: bool,
    /// TESTS ONLY: the fake child answers from an oracle (`closeloop::proofs::ProofOracle`).
    #[serde(default)]
    pub fake: Option<FakeProof>,
    /// lane closeloop3 (closeloop.md 16): EXACT coefficient facts of the records `proof.program` cannot instantiate
    /// (elided witness loci), from the records' own coefficient text (`rr_coef_facts`, Symbolica). None = such
    /// records get no fact job (their positive-numerator terms stay undecided, as before).
    #[serde(default)]
    pub exact_eval: Option<ExactEvalCfg>,
}

/// The `coefficient-facts` jobs of a node (DATA; lane closeloop3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExactEvalCfg {
    /// Key of `spec.programs` (rr_coef_facts, a build of crate rustred-campaign-algebra).
    pub program: String,
    /// (record, key) pairs of one job (records of one table).
    #[serde(default = "d_exact_keys")]
    pub max_keys_per_job: usize,
    #[serde(default = "d_cap")]
    pub cap_bytes: u64,
    #[serde(default = "d_peak")]
    pub expected_peak_bytes: u64,
    /// Wall cap of one job (a legacy child: the supervisor's silence timeout is the cap).
    #[serde(default = "d_proof_wall")]
    pub wall_s: u32,
    /// TESTS ONLY: the fake child emulates the job (facts from the campaign core's exact evaluation; an optional
    /// oracle of lies keyed by key, as `proof.fake`).
    #[serde(default)]
    pub fake: Option<FakeExact>,
}

/// TESTS ONLY: fake `coefficient-facts` answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeExact {
    #[serde(default)]
    pub oracle: Option<PathBuf>,
    #[serde(default)]
    pub ms: u64,
    #[serde(default = "d_fake_program")]
    pub program: String,
}

fn d_exact_keys() -> usize {
    256
}

impl ExactEvalCfg {
    /// The heartbeat value whose silence timeout (3 hb + 5 s) is the wall cap.
    pub fn heartbeat_s(&self) -> u32 {
        (self.wall_s.saturating_sub(5) / 3).max(1)
    }
    /// The program key the jobs run (the fake's in tests).
    pub fn program_key(&self) -> &str {
        self.fake
            .as_ref()
            .map(|f| f.program.as_str())
            .unwrap_or(self.program.as_str())
    }
}

/// TESTS ONLY: fake rr_proof answers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FakeProof {
    pub oracle: PathBuf,
    #[serde(default)]
    pub ms: u64,
    #[serde(default = "d_fake_program")]
    pub program: String,
}

fn d_rows() -> String {
    "engine".into()
}
fn d_span_keys() -> usize {
    64
}
fn d_fact_keys() -> usize {
    4096
}
fn d_proof_wall() -> u32 {
    3600
}

impl ProofCfg {
    /// The heartbeat value whose silence timeout (3 hb + 5 s) is the wall cap.
    pub fn heartbeat_s(&self) -> u32 {
        (self.wall_s.saturating_sub(5) / 3).max(1)
    }
    /// The program key the jobs run (the fake's in tests).
    pub fn program_key(&self) -> &str {
        self.fake
            .as_ref()
            .map(|f| f.program.as_str())
            .unwrap_or(self.program.as_str())
    }
}

fn d_threads() -> usize {
    4
}

impl NodeCfg {
    pub fn base(&self) -> PathBuf {
        self.import_base.clone().unwrap_or_else(|| {
            self.import_spec
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("/"))
        })
    }
}

impl CloseLoopConfig {
    pub fn from_json(text: &str) -> Result<CloseLoopConfig, String> {
        let c: CloseLoopConfig =
            serde_json::from_str(text).map_err(|e| format!("closeloop config: {e}"))?;
        c.validate()?;
        Ok(c)
    }

    pub fn read(path: &Path) -> Result<CloseLoopConfig, String> {
        let t = std::fs::read_to_string(path)
            .map_err(|e| format!("closeloop config {}: {e}", path.display()))?;
        Self::from_json(&t)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema != CONFIG_SCHEMA {
            return Err(format!(
                "closeloop config schema {:?}, expected {CONFIG_SCHEMA}",
                self.schema
            ));
        }
        if self.nodes.is_empty() {
            return Err("closeloop config lists no node".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for n in &self.nodes {
            if n.node.is_empty() {
                return Err("a node without a name".into());
            }
            if !seen.insert(super::ids::safe(&n.node)) {
                return Err(format!(
                    "node {:?} twice (or two names with one safe form)",
                    n.node
                ));
            }
            if n.solve.max_keys_per_job == 0 || n.solve.threads == 0 || n.check_threads == 0 {
                return Err(format!(
                    "node {}: max_keys_per_job, threads and check_threads must be >= 1",
                    n.node
                ));
            }
            if !matches!(n.reference.mismatch.as_str(), "block" | "report") {
                return Err(format!(
                    "node {}: reference.mismatch {:?} (block | report)",
                    n.node, n.reference.mismatch
                ));
            }
            if n.budgets.max_job_attempts == 0 {
                return Err(format!("node {}: max_job_attempts must be >= 1", n.node));
            }
            if !matches!(
                n.solve.relation_rule.as_str(),
                "m1_first" | "first_admissible"
            ) {
                return Err(format!(
                    "node {}: relation_rule {:?} (m1_first | first_admissible)",
                    n.node, n.solve.relation_rule
                ));
            }
            if let Some(p) = &n.proof {
                if p.max_keys_per_job == 0
                    || p.max_fact_keys_per_job == 0
                    || p.threads == 0
                    || p.exact_eval
                        .as_ref()
                        .is_some_and(|x| x.max_keys_per_job == 0)
                {
                    return Err(format!(
                        "node {}: proof batch sizes and threads must be >= 1",
                        n.node
                    ));
                }
                match p.rows.as_str() {
                    "engine" if p.family.is_none() => {
                        return Err(format!(
                            "node {}: proof.rows engine needs proof.family",
                            n.node
                        ));
                    }
                    "cert" if p.cert.is_none() => {
                        return Err(format!("node {}: proof.rows cert needs proof.cert", n.node));
                    }
                    "engine" | "cert" => {}
                    r => return Err(format!("node {}: proof.rows {r:?} (engine | cert)", n.node)),
                }
            }
            if let Some(sc) = &n.scale
                && (sc.store_chunk_files == 0 || sc.store_threads == 0 || sc.store_budget_mib == 0)
            {
                return Err(format!(
                    "node {}: scale.store_chunk_files, store_threads and store_budget_mib must be >= 1",
                    n.node
                ));
            }
            if let Some(e) = &n.escalation {
                for st in &e.steps {
                    rustred_campaign_core::import::recipe_steps::parse_step(st)
                        .map_err(|x| format!("node {}: escalation: {x}", n.node))?;
                }
            }
            if let Some(r) = &n.required_evidence
                && !matches!(r.as_str(), "imported" | "replay" | "per-key")
            {
                return Err(format!("node {}: required_evidence {r:?}", n.node));
            }
        }
        Ok(())
    }
}
