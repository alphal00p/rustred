//! The REPAIR POLICY (interfaces/repairflow.md section 2): every threshold, budget, escalation step, weight and
//! tie-break of the table-vs-recipe decision is a named field here, with a documented default; no constant in the code
//! decides. Effective policy = built-in defaults <- campaign `repair_policy` <- `nodes.<node>` (deep merge of JSON
//! objects; unknown fields refused). Nothing here knows a node, an arity or a count (L8).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

use crate::poles::params::CriteriaOrder;
use crate::poles::{Candidate, MaxSizeRatio, select};

/// Schema of the policy file (campaign input role `repairflow`).
/// v2 = the defaults of the user ruling of 2026-11-19 (rule 21: `keep.choice_order` puts `regular_at` after size, as a
/// tie-break); v1 files (the withdrawn rule's order) still parse and keep what they name.
pub const POLICY_SCHEMA: &str = "rustred.campaign.repairflow.v2";
pub const POLICY_SCHEMA_V1: &str = "rustred.campaign.repairflow.v1";

/// The policy of one node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RepairPolicy {
    /// false = the flow is off for the node (the closing loop alone).
    pub enabled: bool,
    /// Shadowing test before any symbolic search (R31 evidence 04:37Z).
    pub shadow_test: bool,
    pub route0: Route0Policy,
    pub composite: CompositePolicy,
    /// wireflow (b): the two-row step of the composite tool (default off; omitted from the digest while default).
    #[serde(skip_serializing_if = "TwoRowStep::is_default")]
    pub tworow: TwoRowStep,
    /// wireflow (d): lane pieces' PLANNER on every composite round (default off; omitted from the digest while default).
    #[serde(skip_serializing_if = "PiecesStep::is_default")]
    pub pieces: PiecesStep,
    /// wireflow (f): seed minimisation of the closing face recipes at P5 (default off; omitted from the digest).
    #[serde(skip_serializing_if = "SeedMinStep::is_default")]
    pub seedmin: SeedMinStep,
    /// wireflow (c): the composite rounds' region fits SHIPPED as case domains at P5 (default off; omitted from the
    /// digest while default).
    #[serde(skip_serializing_if = "ShipRegionsStep::is_default")]
    pub ship_regions: ShipRegionsStep,
    /// wireflow (g): the regular terminal set R as a phase-D diagnostic job at P5 (default off; omitted from the digest).
    #[serde(skip_serializing_if = "RegBasisStep::is_default")]
    pub regbasis: RegBasisStep,
    /// wireflow (e): coreseed's budgeted VARIANT PHASE after the close-first solves, at P5 (default off; omitted).
    #[serde(skip_serializing_if = "VariantsStep::is_default")]
    pub variants: VariantsStep,
    pub regions: RegionsPolicy,
    /// Extra evidence added to every spec the flow composes (typed slot: zero-fact sources of lane zeros2).
    pub evidence: ExtraEvidence,
    pub faces: FacesPolicy,
    pub symbolic: SymbolicPolicy,
    pub keep: KeepPolicy,
    pub node_budget: NodeBudget,
    pub cost: CostModel,
    pub recipes: RecipePolicy,
    pub proposals: ProposalPolicy,
    pub runtime: RuntimePolicy,
    /// lane gencheck (emission contract): the symbolic table's kept records go through the emission gate (checker on
    /// their own bytes with the certificate of their face lift, class, policy) before it is written; a refused record
    /// stops the node (fail closed). Default off; omitted from the digest while off.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub emission: Option<crate::emission::Settings>,
}

impl Default for RepairPolicy {
    fn default() -> Self {
        RepairPolicy {
            enabled: true,
            shadow_test: true,
            route0: Route0Policy::default(),
            composite: CompositePolicy::default(),
            tworow: TwoRowStep::default(),
            pieces: PiecesStep::default(),
            seedmin: SeedMinStep::default(),
            ship_regions: ShipRegionsStep::default(),
            regbasis: RegBasisStep::default(),
            variants: VariantsStep::default(),
            regions: RegionsPolicy::default(),
            evidence: ExtraEvidence::default(),
            faces: FacesPolicy::default(),
            symbolic: SymbolicPolicy::default(),
            keep: KeepPolicy::default(),
            node_budget: NodeBudget::default(),
            cost: CostModel::default(),
            recipes: RecipePolicy::default(),
            proposals: ProposalPolicy::default(),
            runtime: RuntimePolicy::default(),
            emission: None,
        }
    }
}

/// ROUTE 0 (coordinator 2026-11-14): an exact symmetry image before any generate job. v1 = POINTWISE: the import's
/// verified symmetry credits (`symmetry_credit` source, group derived from the family and proved, `CreditPolicy::
/// from_verified`) serve a key through the image's rule at that key (descent and evidence checked per key). The
/// all-layers version (image domain contained in one record's domain for every layer) is NOT implemented.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Route0Policy {
    /// Add the verified symmetry-credit source to the node's spec (when it has none).
    pub symmetry_credit: bool,
    /// Family file of the group (null = derived from the frame table header's family).
    pub family: Option<String>,
    /// ROUTE 0b: the exact multi-term symmetry relation at the key, every element alone, the smallest DESCENDING
    /// relation (full node comparator) kept (`repairflow::symrel`); served keys leave the symbolic step.
    pub symmetry_relation: bool,
    /// Keys tried by route 0b per node (in key order); beyond: not tried (recorded).
    pub max_keys: u64,
    /// Bound of one expanded symmetry row (terms).
    pub max_terms: usize,
    /// The SHIPPED composite relations of the symmetry-composite step (`rr_rf_symflow` result: its
    /// `composite.table.jsonl` + `composite.verification.jsonl`, pinned): added to the node's spec as a
    /// `supplied_relations` source (the import checks descent; the verification is cited) before faces are formed and
    /// shipped first in the extension. Until the contract kind `symmetry-composite` runs the step as a child.
    pub composite_input: Option<CompositeInput>,
    /// wireflow (a): the composite step run as CHILD JOBS of kind `symmetry-composite` issued by the repair flow itself
    /// (rounds over the node's uncovered keys before its faces are formed), instead of (or after) a pinned result.
    /// Default off: omitted from the canonical policy (digest) while off, so existing campaigns are unchanged.
    #[serde(skip_serializing_if = "CompositeJobs::is_default")]
    pub composite_jobs: CompositeJobs,
}

/// wireflow (a), interfaces/repairflow.md 20: the composite step as child jobs. Round k: the import check of the node's
/// spec with the composites of rounds < k; its UNCOVERED keys not asked before go to ONE `symmetry-composite` job (keys
/// grouped by face); the relations that pass the supervisor's same-backend filter become the round's
/// `supplied_relations` source (`rf:symmetry-composite:r<k>`), re-verified by the import at load like any supplied
/// relation. Rounds stop when no new key is uncovered or after `max_rounds` (rounds = layers: a round's relations reach
/// new keys, TRICKS 10). With `kernel` set, every accepted relation also gets a `proof-symrows` job (symcert's adapter)
/// and the certified ones enter the import with their sealed certificate (per-key, closeloop.md 24).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CompositeJobs {
    pub enabled: bool,
    /// Rounds at most. Default 4: the T162 layered walk on D + r <= 10 met new keys in 2 layers and closed at the 3rd
    /// (notes/implementer.repairflow.md 06:46Z); 4 leaves one round of margin. Not an optimum, a measured bound.
    pub max_rounds: u32,
    /// Key of the campaign spec's `programs`: the composite tool (`rr_rf_symflow`).
    pub program: String,
    pub threads: u32,
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    /// Wall cap of one round's job (s). The tool is a LEGACY child (no heartbeat events): the supervisor's silence
    /// timeout (3 hb + 5 s) is this cap (hb = (wall_s - 5) / 3, as the closing loop's proof jobs). Default 14400:
    /// a bound, not a measurement (T162 D + r <= 10, 2,624 keys: 4 min on 2 threads, notes/implementer.repairflow.md).
    pub wall_s: u64,
    /// Attempts of one round's job after a resource end (signal, OOM, host errno) before the item needs the operator.
    pub max_attempts: u32,
    /// The kernel route (second route of every relation; the only route of two-row forms, audit F8). None = the
    /// same-backend route alone (level `imported`).
    pub kernel: Option<KernelJobs>,
}

impl Default for CompositeJobs {
    fn default() -> Self {
        CompositeJobs {
            enabled: false,
            max_rounds: 4,
            program: "rr_rf_symflow".into(),
            threads: 1,
            cap_bytes: 16 << 30,
            expected_peak_bytes: 4 << 30,
            wall_s: 14_400,
            max_attempts: 2,
            kernel: None,
        }
    }
}

impl CompositeJobs {
    pub fn is_default(&self) -> bool {
        *self == CompositeJobs::default()
    }
    /// The heartbeat value whose silence timeout (3 hb + 5 s) is the wall cap.
    pub fn heartbeat_s(&self) -> u32 {
        (self.wall_s.saturating_sub(5) / 3).clamp(1, u32::MAX as u64) as u32
    }
}

/// The proof kernel of the composite rounds (`rr_proof --symrows`, contract kind `proof-symrows`; interfaces/
/// proof-symrows-job.md): ONE job per accepted relation. Availability of the kernel route = this configuration
/// (closeloop.md 24.9): the program key and its SHA-256 (the import's `kernel_certificates.kernels` allow-list entry);
/// a file at the program's path with another SHA-256 is NO kernel (the round records the reason, same-backend only).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelJobs {
    /// Key of the campaign spec's `programs`.
    pub program: String,
    /// SHA-256 of the kernel binary (hex).
    pub sha256: String,
    /// The kernel's capacities (DATA of the binary; None = no limit stated).
    #[serde(default)]
    pub max_rows: Option<usize>,
    #[serde(default)]
    pub max_seeds: Option<usize>,
    #[serde(default)]
    pub max_primes: Option<u32>,
    /// Zero-sector census written into every relation table: `frame` (the frame table's `zero_sectors` record, as
    /// symcert's call sites) or `none`.
    #[serde(default = "d_census")]
    pub census: String,
    /// Also certify PLAIN relations (one element, no catalogue rule). Default false: the import re-derives a plain
    /// relation exactly from its OWN verified group at load (route (a), per-key, import.md 21), so the kernel adds a
    /// third check at one job per relation; composites (same-backend only otherwise) and two-row forms (kernel only)
    /// are always put to the kernel.
    #[serde(default)]
    pub plain: bool,
    #[serde(default = "d_kernel_cap")]
    pub cap_bytes: u64,
    #[serde(default = "d_kernel_peak")]
    pub expected_peak_bytes: u64,
    /// Wall cap of one kernel job (s; legacy child: silence timeout = this cap). Default 1800: a bound; measured
    /// 0.07-3.7 s per relation on T162 / T178 (notes/implementer.symcert.md).
    #[serde(default = "d_kernel_wall")]
    pub wall_s: u64,
}

impl KernelJobs {
    /// The heartbeat value whose silence timeout (3 hb + 5 s) is the wall cap.
    pub fn heartbeat_s(&self) -> u32 {
        (self.wall_s.saturating_sub(5) / 3).clamp(1, u32::MAX as u64) as u32
    }
}

fn d_census() -> String {
    "frame".into()
}
fn d_kernel_cap() -> u64 {
    4 << 30
}
fn d_kernel_peak() -> u64 {
    1 << 30
}
fn d_kernel_wall() -> u64 {
    1800
}

/// wireflow (b), TRICKS 8 / 1x8: the TWO-ROW step of the composite tool (lane tworow's `two_row_relations` at the keys
/// with no single-element choice, then the composite with one catalogue rule on the near misses). Two-row forms enter
/// the import ONLY through a kernel certificate (audit F8, import.md 21 (b)). Default off (omitted from the digest).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct TwoRowStep {
    pub enabled: bool,
    pub policy: crate::tworow::TwoRowPolicy,
}

/// wireflow (d), TRICKS 4: lane pieces' PLANNER (`pieces::plan`) on every composite round: one chosen service per key of
/// the round (all faces together; candidates = the tool's per-key record of all acceptable choices, `KeyChoices`), the
/// three pieces per face, per-KEY acyclicity, the piece order (coupled faces = one cyclic piece group) and the keys left
/// to the IBP core. The plan is bound to the table set (`check_plan_table_set`, audit G2) and recorded with the round;
/// the face jobs of P1 are then issued in piece order (earlier group first). A record and an order, never a condition
/// of serving a key (the import and the closing loop judge every relation).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct PiecesStep {
    pub plan: bool,
    pub ranking: crate::pieces::plan::Ranking,
    pub options: crate::pieces::keychoices::KeyChoicesOptions,
}

/// wireflow (f), TRICKS 7 / 15 (iii): after the closing loop's final result (P5), lane seedmin's JOINT minimisation
/// (`seedmin::minimise_joint`, objective per the minimiser policy: since rule 21 terms first, the pole order a tie-break;
/// only the explicit pole-first objective refuses a drop that worsens a key's pole order) of the seed list of every EXPLICIT-OFFSET face recipe (the mixed-shell family) that
/// certified at least `min_keys` keys, over exactly those keys, solved by the supervisor-side child solver
/// (`closeloop::seedmin::ChildTrialSolver`, the solve-recipe contract, journaled). When ONE piece holds every key the
/// minimised list becomes the recipe that is SHIPPED in the export (its every-key final verification is the claim);
/// otherwise the full list is shipped and the pieces are recorded. The record compares the minimised and the full list
/// per key (seeds, pole orders). Never changes a closure or a verdict.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SeedMinStep {
    pub enabled: bool,
    pub policy: crate::seedmin::SeedMinPolicy,
    /// Recipes that certified fewer keys are not minimised. Default 2 (a list minimised at one key overfits, TRICKS 5).
    pub min_keys: usize,
    /// Recipes minimised at most (largest key sets first). Default 16: a bound, no measurement.
    pub max_recipes: usize,
    /// Solve children at once. Default 4 (the host rule's thread cap).
    pub jobs: usize,
    /// Wall cap of one solve child (s). Default 1800 (the closing loop's per-key budget).
    pub wall_cap_s: u64,
}

impl Default for SeedMinStep {
    fn default() -> Self {
        SeedMinStep {
            enabled: false,
            policy: Default::default(),
            min_keys: 2,
            max_recipes: 16,
            jobs: 4,
            wall_cap_s: 1800,
        }
    }
}

impl SeedMinStep {
    pub fn is_default(&self) -> bool {
        *self == SeedMinStep::default()
    }
}

/// wireflow (c), TRICKS 2: at P5 the composite rounds' REGION FITS (lane regions' `regionfit` on the tool's per-key
/// choices, sealed `regions.json`) are SHIPPED as case domains (condition = the region's atoms, choice = its element /
/// image rule) when the fit is EXACT (no residual, no unservable key) on the face's keys, EVERY one of those keys is
/// certified in the final statement, and the closing loop's SHIP GATE passed (`final/ship-gate.json`). A region is a
/// generalisation, never a proof: whoever uses it verifies the relation per key at use. Never changes a verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ShipRegionsStep {
    pub enabled: bool,
    /// Ship only when the ship gate passed. Default true; false is for tests of a node whose gate cannot run (Tier K).
    pub require_gate: bool,
}

impl Default for ShipRegionsStep {
    fn default() -> Self {
        ShipRegionsStep {
            enabled: false,
            require_gate: true,
        }
    }
}

impl ShipRegionsStep {
    pub fn is_default(&self) -> bool {
        *self == ShipRegionsStep::default()
    }
}

/// wireflow (g), POLES_OBJECTIVE.md 2 (B) / 3.5: lane regbasis' REGULAR TERMINAL SET R of a CLOSED node as a phase-D
/// DIAGNOSTIC: at P5 one job of kind `regular-basis` (`rr_regbasis`, legacy-wrapped) on the closing loop's final
/// statement and effective import spec; its export object (R + the exact map) and tower record are written NEXT TO THE
/// STATEMENT (`final/regbasis/`). Never changes a closure, a statement or a verdict; a job that gives nothing leaves the
/// node without R (never attention).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RegBasisStep {
    pub enabled: bool,
    /// Key of `spec.programs` (default `rr_regbasis`).
    pub program: String,
    /// rr_regbasis' bounds (allow-list: `--max-depth`, `--exact-max-keys`, `--cache-mb`).
    pub args: Vec<String>,
    /// Memory cap / expected peak of the job. Defaults 4 GiB / 2 GiB: bounds, no measurement.
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    /// Wall cap of the legacy child (s); the silence timeout follows it. Default 3600: a bound, no measurement.
    pub wall_s: u64,
    /// Attempts before the diagnostic gives up (no R; never attention). Default 1.
    pub max_attempts: u32,
}

impl Default for RegBasisStep {
    fn default() -> Self {
        RegBasisStep {
            enabled: false,
            program: "rr_regbasis".into(),
            args: vec![],
            cap_bytes: 4 << 30,
            expected_peak_bytes: 2 << 30,
            wall_s: 3600,
            max_attempts: 1,
        }
    }
}

/// wireflow (e), POLES_OBJECTIVE.md 1 (closing first) / polefix's split: the BUDGETED VARIANT PHASE of the face cores
/// after close-first. The closing loop closed every face key with the FIRST certifying face-recipe step (close-first);
/// at P5, for each closing explicit-offset face recipe with >= `min_keys` certified keys, the face's OTHER
/// explicit-offset steps of the composed spec (the mixed-shell variants: up centres, extra shells; `steps` narrows them)
/// are solved at exactly those keys through the child solver (journaled), within `max_requests` trials per node. Per key
/// the alternatives are ranked by the pole objective (regular at d0 first, then lower pole order, then fewer terms; the
/// closing relation wins ties). When ONE variant is preferred at EVERY key of the face and solved them all, it is
/// SHIPPED as the face's recipe in the export; otherwise the record lists the alternatives. Never changes a closure or a
/// verdict.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct VariantsStep {
    pub enabled: bool,
    /// Variant step names to try (empty = every other explicit-offset step of the face in the composed spec).
    pub steps: Vec<String>,
    /// Trials (one child solve of one variant at the face's keys) per node. Default 64: a bound, no measurement.
    pub max_requests: u64,
    /// Faces with fewer certified keys are skipped. Default 1.
    pub min_keys: usize,
    /// Solve children at once. Default 2.
    pub jobs: usize,
    /// Wall cap of one solve child (s). Default 1800.
    pub wall_cap_s: u64,
}

impl Default for VariantsStep {
    fn default() -> Self {
        VariantsStep {
            enabled: false,
            steps: vec![],
            max_requests: 64,
            min_keys: 1,
            jobs: 2,
            wall_cap_s: 1800,
        }
    }
}

impl VariantsStep {
    pub fn is_default(&self) -> bool {
        *self == VariantsStep::default()
    }
}

impl RegBasisStep {
    pub fn is_default(&self) -> bool {
        *self == RegBasisStep::default()
    }
    /// The legacy child's heartbeat (its silence = the wall cap).
    pub fn heartbeat_s(&self) -> u32 {
        (self.wall_s.saturating_sub(5) / 3).clamp(1, u32::MAX as u64) as u32
    }
}

impl PiecesStep {
    pub fn is_default(&self) -> bool {
        *self == PiecesStep::default()
    }
}

impl TwoRowStep {
    pub fn is_default(&self) -> bool {
        *self == TwoRowStep::default()
    }
}

/// A pinned symmetry-composite result (`route0.composite_input`). The import RE-VERIFIES every relation at load
/// (interfaces/import.md 21: plain by group re-derivation, composites by the same backend from `verification`, or a
/// checked kernel certificate from `kernel_certificates`, lane symfix); nothing here is trusted as evidence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompositeInput {
    pub table: PinnedFile,
    pub verification: PinnedFile,
    /// lane symfix (13.3 (b)): sealed proof-symrows certificate streams + kernel allow-list (the second route).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kernel_certificates: Option<crate::import::supplied_verify::CertificateSpec>,
}

/// A file with its sha256 (verified by the import at load).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PinnedFile {
    pub path: String,
    pub sha256: String,
}

impl Default for Route0Policy {
    fn default() -> Self {
        Route0Policy {
            symmetry_credit: true,
            family: None,
            symmetry_relation: true,
            max_keys: 1_000_000,
            max_terms: 65_536,
            composite_input: None,
            composite_jobs: CompositeJobs::default(),
        }
    }
}

/// COMPOSITE DESCENT (TRICKS 1 + 3; interfaces/repairflow.md 17; `repairflow::composite`): a symmetry service is judged
/// as the composed relation (image row, then the catalogue rule at every image term that is not lower).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CompositePolicy {
    pub enabled: bool,
    /// Catalogue replacements chained below one image term (1 = the image, then one rule).
    pub max_depth: usize,
    /// Safety bound: terms alive during one composition.
    pub max_terms: usize,
    /// Safety bound: replacements of one composition.
    pub max_replacements: usize,
    /// Catalogue records larger than this are not read (the element is refused at that term, recorded).
    pub max_record_bytes: u64,
}

impl Default for CompositePolicy {
    fn default() -> Self {
        CompositePolicy {
            enabled: true,
            max_depth: 1,
            max_terms: 100_000,
            max_replacements: 10_000,
            max_record_bytes: 16 << 20,
        }
    }
}

/// Evidence files the flow adds to the specs it composes (`evidence.facts` of the import spec): the typed slot for a
/// ZERO-FACT source (lane zeros2: structural zero proofs of positive-numerator terms), pinned.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ExtraEvidence {
    pub facts: Vec<PinnedFile>,
}

/// ELEMENT BY REGION (TRICKS 2; lane regions' `regionfit`, interfaces/regionfit.md): fitted per face on the per-key
/// record of all acceptable choices (`KeyChoices`), after the per-key choice; a compression and a record, never a
/// condition of serving a key.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RegionsPolicy {
    pub enabled: bool,
    /// Which identity of a choice the regions are fitted on (form / element / id) and which choices count.
    pub adapter: crate::regionfit::keychoices::KeyChoicesOptions,
    /// The fit itself (atom family, sizes, search budgets; an empty ranking = by the producer's preferred choices).
    pub fit: crate::regionfit::RegionPolicy,
}

impl Default for RegionsPolicy {
    fn default() -> Self {
        RegionsPolicy {
            enabled: true,
            adapter: Default::default(),
            fit: Default::default(),
        }
    }
}

/// How faces are formed (coordinator ruling 2026-11-14: EXCEPTION FACES of the existing parametric rules are the primary
/// object; plain faces (dotted lines x numerators) take the keys no exception face explains).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct FacesPolicy {
    /// Attribute every open key to the first rule whose exception branch cuts it out and search (rule, branch).
    pub exception_faces: bool,
    /// Symbolic attempts on plain faces: `unattributed` (only for keys no parametric exception face explains) | `all`
    /// | `none`. Plain faces always carry the recipe escalation (the pointwise fallback).
    pub plain_faces: String,
    /// Budget of the attribution (domain evaluations over all keys); beyond it the remaining keys are unattributed.
    pub max_attribution_evaluations: u64,
}

impl Default for FacesPolicy {
    fn default() -> Self {
        FacesPolicy {
            exception_faces: true,
            plain_faces: "unattributed".into(),
            max_attribution_evaluations: 2_000_000_000,
        }
    }
}

/// Step S: the cheap symbolic attempt per face (one generate job).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct SymbolicPolicy {
    /// false = NEVER TABLES (every face goes to recipes).
    pub enabled: bool,
    /// `--max-depth` of the face search (shells around each centre).
    pub max_depth: u32,
    /// Centre of the face search: `last_line_floored` | `first_line_floored` | `target` | `numerator` |
    /// `union_floored`.
    pub centre: String,
    /// `--numeric-depth` (per-point search of point faces).
    pub numeric_depth: u32,
    /// Wall cap of one face attempt (s; the time-decided safety net: `budgets.wall.case_s`).
    pub wall_case_s: u64,
    /// The lift PRE-SCREEN (`--screen R,P,D,T --on-heavy refuse`): a relation over these is refused BEFORE its exact
    /// lift (deterministic modular replay of the trace).
    pub screen: Screen,
    /// Cap of faces tried symbolically (in `face_order`); null = all.
    pub max_faces: Option<usize>,
    /// `open_keys_desc` | `open_keys_asc` | `label`.
    pub face_order: String,
    pub threads: u32,
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    pub heartbeat_s: u32,
    pub stop_grace_s: u32,
    /// Extra generate arguments (each must pass the contract's allow-list).
    pub extra_args: Vec<String>,
    /// The size guard of the EXPLICIT regular-first preference (the 2026-11-15 ruling "regular at d = 4 always wins" it
    /// served is withdrawn by rule 21, 2026-11-19): a rational >= 1 or "unbounded" (default). Validated only; no default
    /// path consumes it. The per-rule / per-node caps and the budgets stay separate parameters.
    pub max_size_ratio: SizeRatio,
}

/// `max_size_ratio`: a rational >= 1 or "unbounded".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SizeRatio {
    Ratio(f64),
    Word(String),
}

impl SizeRatio {
    /// The bound, None = unbounded; Err for a word other than "unbounded" or a ratio below 1. (Diagnostic form: the
    /// choice itself uses [`SizeRatio::exact`].)
    pub fn bound(&self) -> Result<Option<f64>, String> {
        match self.exact()? {
            MaxSizeRatio::Unbounded => Ok(None),
            MaxSizeRatio::Ratio { num, den } => Ok(Some(num as f64 / den as f64)),
        }
    }

    /// int7 (B1): the EXACT ratio through the one parser of `max_size_ratio` (`poles::MaxSizeRatio::parse`): a word
    /// ("unbounded", "p/q", "8") as it is; a JSON number by its exact binary value (a finite float is a dyadic
    /// rational: doubling it until it is an integer is exact), refused when it does not fit u64 / u64 or is below 1.
    pub fn exact(&self) -> Result<MaxSizeRatio, String> {
        let text = match self {
            SizeRatio::Word(w) => w.clone(),
            SizeRatio::Ratio(r) => {
                let (mut x, mut den) = (*r, 1u64);
                if !x.is_finite() || x < 1.0 {
                    return Err(format!("max_size_ratio {r}: must be >= 1"));
                }
                while x.fract() != 0.0 {
                    if den >= 1 << 62 {
                        return Err(format!(
                            "max_size_ratio {r}: not an exact rational p/q in u64 (write it as text \"p/q\")"
                        ));
                    }
                    x *= 2.0;
                    den *= 2;
                }
                if x >= u64::MAX as f64 {
                    return Err(format!("max_size_ratio {r}: too large"));
                }
                format!("{}/{den}", x as u64)
            }
        };
        MaxSizeRatio::parse(&Value::String(text)).map_err(|e| e.0)
    }
}

/// The regular-vs-size trade: keep the REGULAR candidate (size `regular`) over the smallest singular one (size
/// `smallest`) iff `regular <= ratio x smallest` (always with "unbounded"). Returns (prefer regular, report line naming
/// the value in force and both sizes). int7 (B1): decided by the ONE selection function (`poles::select`, the special
/// case of `poles::pole_objective`) in exact integer arithmetic.
pub fn prefer_regular(
    regular: u64,
    smallest: u64,
    ratio: &SizeRatio,
) -> Result<(bool, String), String> {
    let exact = ratio.exact()?;
    let cand = |id: &str, size: u64, pole: Option<u64>| Candidate {
        id: id.into(),
        admissible: true,
        excluded_by: None,
        evidence_level: 0,
        size,
        pole_order: pole,
    };
    // the singular candidate's order is not known here: unknown ranks after every regular one, as a singular would
    let cands = [
        cand("regular", regular, Some(0)),
        cand("smallest", smallest.max(1), None),
    ];
    let keep =
        select(&cands, exact, CriteriaOrder::RegularFirst).is_some_and(|c| c.id == "regular");
    Ok((
        keep,
        format!(
            "max_size_ratio {}: regular candidate {regular} terms vs smallest {smallest}: {}",
            exact.text(),
            if keep {
                "regular kept"
            } else {
                "smallest kept (regular candidate exists, over max_size_ratio)"
            }
        ),
    ))
}

impl Default for SymbolicPolicy {
    fn default() -> Self {
        SymbolicPolicy {
            enabled: true,
            max_depth: 2,
            // helper (w13, 2026-11-14): centre = every free slot one step to its floor, floored, depth 2 reproduces the
            // engine's own minimal face rule term for term (T162 sc-c010#1: 6 terms, 50 monomials, 0.1 s)
            centre: "union_floored".into(),
            numeric_depth: 2,
            wall_case_s: 120,
            screen: Screen::default(),
            max_faces: None,
            face_order: "open_keys_desc".into(),
            threads: 1,
            cap_bytes: 2 << 30,
            expected_peak_bytes: 1 << 30,
            heartbeat_s: 10,
            stop_grace_s: 30,
            extra_args: vec![],
            max_size_ratio: SizeRatio::Word("unbounded".into()),
        }
    }
}

/// The engine's lift pre-screen `MIN_ROWS,MAX_POINTS,MAX_DEG,MAX_RHS`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Screen {
    pub min_rows: u64,
    pub max_points: u64,
    pub max_degree: u64,
    pub max_rhs_terms: u64,
}

impl Default for Screen {
    fn default() -> Self {
        Screen {
            min_rows: 0,
            max_points: 64,
            max_degree: 24,
            max_rhs_terms: 2500,
        }
    }
}

impl Screen {
    /// The `--screen` argument.
    pub fn arg(&self) -> String {
        format!(
            "{},{},{},{}",
            self.min_rows, self.max_points, self.max_degree, self.max_rhs_terms
        )
    }
}

/// Per-rule caps and the keys a kept rule must serve.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct KeepPolicy {
    pub max_rhs_terms: u64,
    pub max_monomials: u64,
    pub max_bytes: u64,
    pub max_trace_rows: u64,
    /// A rule credited at fewer open keys of its face goes to recipes.
    pub min_keys_served: u64,
    /// ... or at a smaller share of the face's open keys.
    pub min_served_fraction: f64,
    /// The ONE ordering among several admissible candidates of a face (after `keep` first): criteria in order, from
    /// [`CHOICE_CRITERIA`]; `regular_at` uses `recipes.prefer_regular_at` (reserved: equal until evaluated).
    pub choice_order: Vec<String>,
}

/// Criteria of [`KeepPolicy::choice_order`].
pub const CHOICE_CRITERIA: &[&str] = &[
    "served_desc",
    "regular_at",
    "bytes_asc",
    "rhs_terms_asc",
    "monomials_asc",
    "record_digest",
];

impl Default for KeepPolicy {
    fn default() -> Self {
        KeepPolicy {
            max_rhs_terms: 2500,
            max_monomials: 200_000,
            max_bytes: 2 << 20,
            max_trace_rows: 5000,
            min_keys_served: 2,
            min_served_fraction: 0.0,
            // USER RULING 2026-11-19 (rule 21): size before regularity; `regular_at` breaks ties only (it came before
            // `bytes_asc` under the withdrawn rule)
            choice_order: vec![
                "served_desc".into(),
                "bytes_asc".into(),
                "regular_at".into(),
                "record_digest".into(),
            ],
        }
    }
}

/// Per-node budget of NEW symbolic rules.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct NodeBudget {
    pub max_bytes: u64,
    pub max_rules: u64,
    /// Which candidates get the budget first: `keys_per_byte` | `keys` | `bytes_asc`.
    pub order: String,
}

impl Default for NodeBudget {
    fn default() -> Self {
        NodeBudget {
            max_bytes: 64 << 20,
            max_rules: 1000,
            order: "keys_per_byte".into(),
        }
    }
}

/// The crossover cost model (section 3 of the interface).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct CostModel {
    pub enabled: bool,
    pub load_ns_per_byte: f64,
    pub apply_ns_base: f64,
    pub apply_ns_per_monomial: f64,
    pub recipe_solve_s: f64,
    pub cached_ns_base: f64,
    pub cached_ns_per_rhs_term: f64,
    pub cache_hit_fraction: f64,
    pub uses_per_served_key: f64,
    /// Extra expected uses per FREE DIRECTION of a symbolic rule (a slot symbolic in its case: the rule holds for
    /// every power along it, also outside the certified set where run-time inputs arrive). Default 1e6: a rule valid
    /// on the whole face (within the caps) is preferred to ANY number of point relations on it (coordinator ruling
    /// 2026-11-14; helper growth data: arrival sets do not saturate, 28 -> 924 inputs = x34 demand at T178).
    pub unbounded_direction_uses: f64,
    pub table_weight: f64,
}

impl Default for CostModel {
    fn default() -> Self {
        CostModel {
            enabled: true,
            // MEASURED (lane repairflow timing, unified frozen engine, T178 kept rules 61 KB / 407 KB / 1.53 MB,
            // rr_proof instantiate = exact evaluation of every coefficient at the key): per key 1.75 / 6.2 / 11.2 ms ->
            // slope 82 ns per monomial, base 1.3 ms; load ~15 ns per byte; face recipe solved with the recipe known
            // (d2 last line floored) 16-47 s per key (median 36 s); a cached relation (point record re-evaluated)
            // <= 20 ms per key beyond process start (1 and 1,144 terms): base 1 ms + 10 us per term (provisional split)
            load_ns_per_byte: 15.0,
            apply_ns_base: 1_300_000.0,
            apply_ns_per_monomial: 82.0,
            recipe_solve_s: 36.0,
            cached_ns_base: 1_000_000.0,
            cached_ns_per_rhs_term: 10_000.0,
            cache_hit_fraction: 0.9,
            uses_per_served_key: 1.0,
            unbounded_direction_uses: 1_000_000.0,
            table_weight: 1.0,
        }
    }
}

/// Step R: the recipe escalation per face.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RecipePolicy {
    /// false = NEVER RECIPES from this flow (the closing loop's default recipe still applies).
    pub enabled: bool,
    /// false = no per-face recipes, only the default recipe (R49).
    pub per_face: bool,
    /// Step names (import.md 14 grammar), tried in order per key.
    pub escalation: Vec<String>,
    /// Centre choice priorities; null = the node order's coordinate priority.
    pub line_priority: Option<Vec<usize>>,
    pub numerator_priority: Option<Vec<usize>>,
    /// Cap of faces given per-face recipes (in `symbolic.face_order`); null = all.
    pub max_faces: Option<usize>,
    /// Shells of the `mixed_shell` step, in the harder unit (+1 line, -1 numerator): explicit offsets (lane termmin2).
    pub mixed_shells: Vec<String>,
    /// Tie-break among ADMISSIBLE relations / candidates only: prefer the one regular at these dimensions (USER ruling:
    /// ON by default with [4]; [] = off). Never changes whether a node closes; recorded when it changes a choice.
    /// RESERVED: not evaluated yet (needs the engine's fixed-d solve entry, interfaces/pole-diagnostics.md): the
    /// criterion `regular_at` of [`KeepPolicy::choice_order`] compares equal until the data exists.
    pub prefer_regular_at: Vec<i64>,
}

impl Default for RecipePolicy {
    fn default() -> Self {
        RecipePolicy {
            enabled: true,
            per_face: true,
            escalation: vec![
                // prior (helper k13 classification of 8,215 unserved T162 arrival keys): ~78 % symmetry only, ~22 %
                // depth 2 (last line 1,477, union 320), depth 3 rare (9): depth 3 only under an explicit policy
                "d0_cnone_foff_ssym_mM1C".into(),
                // the mixed-shell family (helper: one generation rule closed every T162 core tried; lane termmin2
                // reproduced K* with it); before the depth-2 steps
                "mixed_shell".into(),
                "d2_clast_fon_mM1C".into(),
                "d2_cnum_fon_mM1C".into(),
                "d2_cunion_fon_mM1C".into(),
            ],
            line_priority: None,
            numerator_priority: None,
            max_faces: None,
            mixed_shells: ["S0", "S1", "Sm1", "S1m1", "S1m2", "Sm2"]
                .iter()
                .map(|x| x.to_string())
                .collect(),
            prefer_regular_at: vec![4],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct ProposalPolicy {
    pub enabled: bool,
    /// A key no step of the escalation (nor the default recipe) serves ends as a TERMINAL PROPOSAL; with this flag the
    /// proposals are DECLARED terminals in a re-check so the node closes when everything else is served (user ruling
    /// 2026-11-14: a further terminal is acceptable); false = they stay open.
    pub declare_terminal: bool,
    /// At most this many proposals are declared (the terminal set must stay finite and small); above: none declared.
    pub max_terminals: usize,
    /// Keys KNOWN to have no relation within the policy (spec data): labelled `known_open` instead of new proposals.
    pub known_open: Vec<Vec<i32>>,
}

impl Default for ProposalPolicy {
    fn default() -> Self {
        ProposalPolicy {
            enabled: true,
            // directing session 2026-11-14: keys nothing serves become terminal PROPOSALS awaiting an operator
            // decision, NEVER automatic terminals: the close-out by declaration is opt-in
            declare_terminal: false,
            max_terminals: 100,
            known_open: vec![],
        }
    }
}

/// Run-time budgets written into the export manifest (the consumer solving recipes at keys outside the certified set).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RuntimePolicy {
    /// Recipe solves per reduction input before a typed error (`resource limit`).
    pub max_solves_per_input: u64,
    /// Wall seconds of recipe solves per reduction input before a typed error.
    pub max_seconds_per_input: u64,
    /// Wall seconds of one solve at one key (the solve-recipe `--key-budget-s`).
    pub max_seconds_per_key: u64,
}

impl Default for RuntimePolicy {
    fn default() -> Self {
        RuntimePolicy {
            max_solves_per_input: 10_000,
            max_seconds_per_input: 3600,
            max_seconds_per_key: 600,
        }
    }
}

/// The policy FILE: campaign section + per-node overrides.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyFile {
    pub schema: String,
    #[serde(default)]
    pub repair_policy: Value,
    #[serde(default)]
    pub nodes: BTreeMap<String, Value>,
}

/// Deep merge of JSON objects (`over` wins; arrays and scalars are replaced).
pub fn merge(base: &mut Value, over: &Value) {
    match (base, over) {
        (Value::Object(b), Value::Object(o)) => {
            for (k, v) in o {
                match b.get_mut(k) {
                    Some(bv) if bv.is_object() && v.is_object() => merge(bv, v),
                    _ => {
                        b.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (b, o) => *b = o.clone(),
    }
}

impl PolicyFile {
    pub fn parse(text: &str) -> Result<PolicyFile, String> {
        let f: PolicyFile =
            serde_json::from_str(text).map_err(|e| format!("repair policy: {e}"))?;
        if f.schema != POLICY_SCHEMA && f.schema != POLICY_SCHEMA_V1 {
            return Err(format!(
                "repair policy: schema {:?}, expected {POLICY_SCHEMA} (or {POLICY_SCHEMA_V1})",
                f.schema
            ));
        }
        // every section must deserialize on its own (unknown fields refused early, per node too)
        f.effective(None)?;
        for n in f.nodes.keys() {
            f.effective(Some(n))?;
        }
        Ok(f)
    }

    /// The effective policy of `node` (None = the campaign section only).
    pub fn effective(&self, node: Option<&str>) -> Result<RepairPolicy, String> {
        let mut v = serde_json::to_value(RepairPolicy::default()).expect("policy json");
        if !self.repair_policy.is_null() {
            merge(&mut v, &self.repair_policy);
        }
        if let Some(n) = node
            && let Some(o) = self.nodes.get(n)
        {
            merge(&mut v, o);
        }
        let p: RepairPolicy = serde_json::from_value(v).map_err(|e| {
            format!(
                "repair policy{}: {e}",
                node.map(|n| format!(" of node {n}")).unwrap_or_default()
            )
        })?;
        p.validate()?;
        Ok(p)
    }
}

impl RepairPolicy {
    /// Sanity of the values (a usage error, never a silent clamp).
    pub fn validate(&self) -> Result<(), String> {
        let frac = |x: f64, what: &str| {
            if (0.0..=1.0).contains(&x) {
                Ok(())
            } else {
                Err(format!("repair policy: {what} = {x} not in [0, 1]"))
            }
        };
        frac(self.keep.min_served_fraction, "keep.min_served_fraction")?;
        frac(self.cost.cache_hit_fraction, "cost.cache_hit_fraction")?;
        for (x, what) in [
            (self.cost.load_ns_per_byte, "cost.load_ns_per_byte"),
            (self.cost.apply_ns_base, "cost.apply_ns_base"),
            (
                self.cost.apply_ns_per_monomial,
                "cost.apply_ns_per_monomial",
            ),
            (self.cost.recipe_solve_s, "cost.recipe_solve_s"),
            (self.cost.cached_ns_base, "cost.cached_ns_base"),
            (
                self.cost.cached_ns_per_rhs_term,
                "cost.cached_ns_per_rhs_term",
            ),
            (self.cost.uses_per_served_key, "cost.uses_per_served_key"),
            (
                self.cost.unbounded_direction_uses,
                "cost.unbounded_direction_uses",
            ),
            (self.cost.table_weight, "cost.table_weight"),
        ] {
            if !x.is_finite() || x < 0.0 {
                return Err(format!(
                    "repair policy: {what} = {x} must be finite and >= 0"
                ));
            }
        }
        if !matches!(
            self.symbolic.centre.as_str(),
            "last_line_floored" | "first_line_floored" | "target" | "numerator" | "union_floored"
        ) {
            return Err(format!(
                "repair policy: symbolic.centre {:?} unknown",
                self.symbolic.centre
            ));
        }
        if !matches!(
            self.symbolic.face_order.as_str(),
            "open_keys_desc" | "open_keys_asc" | "label"
        ) {
            return Err(format!(
                "repair policy: symbolic.face_order {:?} unknown",
                self.symbolic.face_order
            ));
        }
        if !matches!(
            self.node_budget.order.as_str(),
            "keys_per_byte" | "keys" | "bytes_asc"
        ) {
            return Err(format!(
                "repair policy: node_budget.order {:?} unknown",
                self.node_budget.order
            ));
        }
        for c in &self.keep.choice_order {
            if !CHOICE_CRITERIA.contains(&c.as_str()) {
                return Err(format!(
                    "repair policy: keep.choice_order: unknown criterion {c:?}"
                ));
            }
        }
        if !matches!(
            self.faces.plain_faces.as_str(),
            "unattributed" | "all" | "none"
        ) {
            return Err(format!(
                "repair policy: faces.plain_faces {:?} unknown",
                self.faces.plain_faces
            ));
        }
        self.symbolic.max_size_ratio.exact()?;
        if self.composite.max_depth > 8 {
            return Err("repair policy: composite.max_depth must be <= 8".into());
        }
        let cj = &self.route0.composite_jobs;
        if cj.enabled && (cj.max_rounds == 0 || cj.max_attempts == 0 || cj.program.is_empty()) {
            return Err(
                "repair policy: route0.composite_jobs needs max_rounds >= 1, max_attempts >= 1 and a program".into(),
            );
        }
        if let Some(k) = &cj.kernel {
            if k.sha256.len() != 64 || !k.sha256.bytes().all(|c| c.is_ascii_hexdigit()) {
                return Err(
                    "repair policy: route0.composite_jobs.kernel.sha256 must be 64 hex digits"
                        .into(),
                );
            }
            if !matches!(k.census.as_str(), "frame" | "none") {
                return Err(format!(
                    "repair policy: route0.composite_jobs.kernel.census {:?} unknown (frame | none)",
                    k.census
                ));
            }
        }
        if self.tworow.enabled && !cj.enabled && self.route0.composite_input.is_none() {
            return Err(
                "repair policy: tworow.enabled needs route0.composite_jobs (the step runs inside the composite job)"
                    .into(),
            );
        }
        if self.symbolic.wall_case_s == 0 {
            return Err("repair policy: symbolic.wall_case_s must be > 0".into());
        }
        for s in &self.recipes.escalation {
            if s == super::recipes::MIXED_SHELL {
                continue;
            }
            // lane wireflow (e): the mixed-shell variants mixed_shell[@up][+SHELL]*
            if let Some(v) = super::recipes::mixed_shell_variant(s) {
                v.map_err(|e| format!("repair policy: recipes.escalation: {e}"))?;
                continue;
            }
            crate::import::recipe_steps::parse_step(s)
                .map_err(|e| format!("repair policy: recipes.escalation: {e}"))?;
        }
        for sh in &self.recipes.mixed_shells {
            crate::termmin::steps::parse_shell(sh)
                .map_err(|e| format!("repair policy: recipes.mixed_shells: {e}"))?;
        }
        Ok(())
    }

    /// Canonical JSON of the effective policy (recorded in every decision record).
    pub fn canonical(&self) -> String {
        let v = serde_json::to_value(self).expect("policy json");
        crate::schema::canon::canonical_json(&v, true).unwrap_or_else(|_| v.to_string())
    }

    /// `fnv1a128` digest of [`Self::canonical`] (hex): the policy identity in records and the statement header.
    pub fn digest(&self) -> String {
        format!(
            "{:032x}",
            crate::worklist::items::fnv1a128(self.canonical().as_bytes())
        )
    }
}
