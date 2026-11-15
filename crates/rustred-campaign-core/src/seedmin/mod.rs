//! SEED MINIMISATION per key and JOINT over a domain (interfaces/seedmin.md; TRICKS.md items 5, 7, 14 (c), 15 (iii),
//! 16). The engine minimiser of contract line 7 (`elimination.minimise`) is not coded in any engine: this module
//! EMULATES it through repeated solves of explicit seed lists (one trial = one child job of the existing kind
//! `solve-recipe`), behind the same names, so that the engine verb replaces it without a spec change (backend
//! `engine`).
//!
//! Library only (R22): no process, no file, no clock. Every solve is a [`Trial`] answered by a [`TrialSolver`] (the
//! supervisor's child solver or a test double); decisions are taken in a fixed order from the answers, so a journal of
//! answers ([`journal::JournaledSolver`]) reproduces the record and the number of CPUs never changes a result.
//! Generic (L8): arity, centres, seed lists, orders, measures and budgets are input or policy data.

pub mod accept;
pub mod bridge;
pub mod candidate;
pub mod joint;
pub mod journal;
pub mod judge;
pub mod key;
pub mod order;
pub mod policy;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use candidate::{RuleCandidate, SizeDecider, rule_candidate};
pub use joint::{JointMin, Piece, minimise_joint};
pub use journal::JournaledSolver;
pub use judge::{LocalValuationJudge, StubRelationPoles, StubSystemPoles, facts_from_record};
pub use key::{KeyMin, minimise_key};
pub use policy::SeedMinPolicy;

/// An integral key (one integer per slot).
pub type Key = Vec<i32>;
/// A seed offset (relative to the CENTRE in seed lists; relative to the KEY in engine records).
pub type Offset = Vec<i32>;

/// How a trial is run.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "mode")]
pub enum TrialMode {
    /// A normal solve with exact lift and exact proof.
    Solve,
    /// The engine's probe mode at `d0` (`--probe-d`): modular, DIAGNOSTIC answers only, never a relation.
    Probe { d0: String },
}

/// One solve request: the explicit list (centre-relative offsets, in list order = the elimination's seed order) at
/// the given keys. The seed of offset `o` at key `k` is `k + centre + o`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trial {
    /// Deterministic label (not part of the digest).
    pub label: String,
    pub centre: Offset,
    pub list: Vec<Offset>,
    pub keys: Vec<Key>,
    pub mode: TrialMode,
    /// Backend `engine`: the recipe's line-7 `elimination.minimise` member (one trial returns the minimised relation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub engine_minimise: Option<Value>,
}

impl Trial {
    /// The seeds as offsets FROM THE KEY (`centre + o`), in list order: the explicit recipe's centres.
    pub fn key_offsets(&self) -> Vec<Offset> {
        self.list.iter().map(|o| add(&self.centre, o)).collect()
    }

    /// sha256 of the canonical JSON of the trial's content (centre, list, keys, mode, engine member; NOT the label):
    /// the journal identity.
    pub fn digest(&self) -> String {
        let v = serde_json::json!({"centre": self.centre, "list": self.list, "keys": self.keys,
            "mode": self.mode, "engine_minimise": self.engine_minimise});
        let t = crate::schema::canon::canonical_json(&v, false).unwrap_or_default();
        sha256_hex(t.as_bytes())
    }
}

/// Pole order of a relation (or system) at d0: 0 = regular.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "pole")]
pub enum PoleOrder {
    Order { order: u32 },
    /// Singular, order not known (at least 1): e.g. coreseed's temporary denominator test.
    Singular,
    Undecided,
}

impl PoleOrder {
    pub fn regular() -> PoleOrder {
        PoleOrder::Order { order: 0 }
    }
    pub fn order(&self) -> Option<u32> {
        match self {
            PoleOrder::Order { order } => Some(*order),
            PoleOrder::Singular | PoleOrder::Undecided => None,
        }
    }
}

/// What the minimiser needs from a certified relation (small: journaled; the record itself stays with the solver).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationFacts {
    pub relation_digest: String,
    /// Right-hand-side terms.
    pub rhs_count: u64,
    /// ... of them in proper subsectors (`sub`), when reported.
    #[serde(default)]
    pub rhs_sub_count: Option<u64>,
    /// Bytes of the relation (the record's canonical text), when known.
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub trace_rows: Option<u64>,
    /// The support of the exact proof (seeds with a nonzero weight), CENTRE-RELATIVE, when the engine reports them.
    #[serde(default)]
    pub seeds_used: Option<Vec<Offset>>,
    /// Pole order at d0 (filled by the solver with lane poles' judge before journaling).
    pub pole: PoleOrder,
    /// Backend `engine`: the record's line-7 `minimise` member, verbatim.
    #[serde(default)]
    pub minimise: Option<Value>,
}

/// The per-key answer of a trial.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "answer")]
pub enum TrialAnswer {
    /// SOLVED and certified by the caller's acceptance chain.
    Relation(Box<RelationFacts>),
    /// NO_RELATION_IN_SEEDS (a decided search).
    NoRelation,
    NotInDomain,
    /// Probe mode only: the probe verdict (never a relation, never evidence).
    Probe { verdict: String },
    /// Budget / error / refused / interrupted: UNDECIDED, never "no relation".
    Undecided { class: String, message: String },
}

impl TrialAnswer {
    pub fn word(&self) -> String {
        match self {
            TrialAnswer::Relation(_) => "relation".into(),
            TrialAnswer::NoRelation => "no_relation".into(),
            TrialAnswer::NotInDomain => "not_in_domain".into(),
            TrialAnswer::Probe { verdict } => format!("probe:{verdict}"),
            TrialAnswer::Undecided { class, .. } => format!("undecided:{class}"),
        }
    }
    pub fn relation(&self) -> Option<&RelationFacts> {
        match self {
            TrialAnswer::Relation(r) => Some(r),
            _ => None,
        }
    }
}

/// What the engine behind a solver can do (from `rr_node version --json` features).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineCaps {
    /// Probe mode `--probe-d` (coord-pole-engine.md 2).
    pub probe_d: bool,
    /// Recipe field `elimination.discovery_d` (coord-pole-engine.md 1).
    pub discovery_d: bool,
    /// Recipe field `elimination.minimise` (coord-seed-minimise.md 5).
    pub minimise: bool,
    /// The features as reported (for the record).
    pub features: Vec<String>,
}

/// Feature names (in `rr_node version --json`) that declare each capability. PROPOSED names (no engine has them yet:
/// requested in the notes); the first one present decides.
pub const PROBE_FEATURES: [&str; 2] = ["probe_d", "solve_recipe_probe"];
pub const DISCOVERY_D_FEATURES: [&str; 1] = ["discovery_d"];
pub const MINIMISE_FEATURES: [&str; 2] = ["seed_minimise", "minimise"];

impl EngineCaps {
    /// Capabilities from a feature list.
    pub fn from_features(features: &[String]) -> EngineCaps {
        let has = |names: &[&str]| features.iter().any(|f| names.contains(&f.as_str()));
        EngineCaps {
            probe_d: has(&PROBE_FEATURES),
            discovery_d: has(&DISCOVERY_D_FEATURES),
            minimise: has(&MINIMISE_FEATURES),
            features: features.to_vec(),
        }
    }
}

/// The solver: runs a batch of independent trials, returns per trial the answers in key order.
pub trait TrialSolver {
    fn solve(&mut self, trials: &[Trial]) -> Result<Vec<Vec<TrialAnswer>>, String>;
    fn capabilities(&self) -> EngineCaps;
}

/// Lane poles' narrow call (`pole_order_of(relation, key, d0)`): used by SOLVERS to fill [`RelationFacts::pole`]
/// from the verified record before the answer is journaled.
pub trait RelationPoles {
    fn pole_order_of(&mut self, record: &Value, key: &[i32], d0: &str) -> PoleOrder;
    /// Name for the record.
    fn name(&self) -> String;
}

/// Lane tadic's narrow call (`keeps_regularity(system_before, system_after, K, d0)`): the system's best pole order p*
/// is not worse after the change. Systems are given as seed offsets FROM THE KEY. `None` = undecided.
pub trait SystemPoles {
    /// Consulted only when available (a stub is not: then only the relation's pole order decides).
    fn available(&self) -> bool;
    fn keeps_regularity(
        &mut self,
        before: &[Offset],
        after: &[Offset],
        key: &[i32],
        d0: &str,
    ) -> Option<bool>;
    fn name(&self) -> String;
}

/// Typed errors of the minimiser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "error")]
pub enum SeedMinError {
    /// `rows_at_d0` asked, the engine has no probe mode (TRICKS 14 c needs `--probe-d` / `discovery_d`).
    EngineLacksProbeMode { features: Vec<String> },
    /// Backend `engine` asked, the engine has no `elimination.minimise`.
    EngineLacksMinimiser { features: Vec<String> },
    /// Bad input (empty list, arity mismatch, duplicate keys, ...).
    Input { message: String },
    /// The solver failed (I/O, launcher, interrupted): resume from the journal.
    Solver { message: String },
    /// The solver answered inconsistently (wrong number of answers).
    Protocol { message: String },
}

impl std::fmt::Display for SeedMinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeedMinError::EngineLacksProbeMode { features } => {
                write!(f, "engine lacks probe mode (features {features:?})")
            }
            SeedMinError::EngineLacksMinimiser { features } => {
                write!(f, "engine lacks the seed minimiser (features {features:?})")
            }
            SeedMinError::Input { message } => write!(f, "input: {message}"),
            SeedMinError::Solver { message } => write!(f, "solver: {message}"),
            SeedMinError::Protocol { message } => write!(f, "protocol: {message}"),
        }
    }
}

impl std::error::Error for SeedMinError {}

/// Componentwise sum.
pub fn add(a: &[i32], b: &[i32]) -> Offset {
    a.iter().zip(b).map(|(x, y)| x + y).collect()
}

/// Componentwise difference.
pub fn sub(a: &[i32], b: &[i32]) -> Offset {
    a.iter().zip(b).map(|(x, y)| x - y).collect()
}

/// sha256 hex of bytes.
pub fn sha256_hex(b: &[u8]) -> String {
    let mut h = crate::closure::sha256::Sha256::new();
    h.update(b);
    h.finish_hex()
}

/// sha256 of the canonical JSON of a seed list (the list's identity in labels and records).
pub fn list_digest(list: &[Offset]) -> String {
    let t = crate::schema::canon::canonical_json(&serde_json::json!(list), false).unwrap_or_default();
    sha256_hex(t.as_bytes())
}

/// Key text `[a,b,..]` for labels.
pub fn key_label(k: &[i32]) -> String {
    format!(
        "[{}]",
        k.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",")
    )
}

/// Run one batch through the solver and check the shape of the answers.
pub(crate) fn run_batch(
    solver: &mut dyn TrialSolver,
    trials: &[Trial],
) -> Result<Vec<Vec<TrialAnswer>>, SeedMinError> {
    if trials.is_empty() {
        return Ok(vec![]);
    }
    let out = solver
        .solve(trials)
        .map_err(|message| SeedMinError::Solver { message })?;
    if out.len() != trials.len() {
        return Err(SeedMinError::Protocol {
            message: format!("{} answers for {} trials", out.len(), trials.len()),
        });
    }
    for (t, a) in trials.iter().zip(&out) {
        if a.len() != t.keys.len() {
            return Err(SeedMinError::Protocol {
                message: format!(
                    "trial {}: {} answers for {} keys",
                    t.label,
                    a.len(),
                    t.keys.len()
                ),
            });
        }
    }
    Ok(out)
}
