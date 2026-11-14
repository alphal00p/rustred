//! The ACCEPTANCE CHAIN for one candidate rule in the context of a node (interface `interfaces/acceptance.md`;
//! plan 2.4.5, 2.4.7 D3/D6/D7, 2.5, 2.6; R8, R9, R13 A6, R29, R33, R38, R40 U-9/U-12).
//!
//! `accept_candidate` runs the ordered checks Record -> Served -> Coverage -> Regularity -> Descent -> Evidence ->
//! Exclusions -> Scope and returns ONE verdict with an evidence record the ledger can cite. A LIBRARY (R22): external
//! tools enter only through the back-end traits ([`facts::Regularity`], [`facts::CoefficientFacts`],
//! [`prover::EvidenceProver`]); a file-backed back-end without an answer makes the chain return `NeedsExternal` with
//! the request files to write. No master count, knock-out flag or old residual count enters anywhere (U-9).

pub mod descent;
pub mod facts;
pub mod ladder_adapters;
pub mod prover;
pub mod recipe;
pub mod record;
pub mod rule;

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

use crate::closure::bodies::Sectors;
use crate::cover::{Dispatch, Dispatcher};
use crate::domain::Stage;
use crate::key::{Key, NodeFrame};
use crate::order::NodeComparator;
use crate::partition::{Containment, contained_in};

use descent::{DescentAt, DescentFailure, Instantiator, descent_at, symbolic_not_proved};
use facts::{CoefficientFacts, RegVerdict, Regularity};
use prover::{EvidenceArtefact, EvidenceProver, ExternalRequest, KeyFailure, KeyStatus, ProverAnswer};
pub use prover::RequiredEvidence;
use record::AcceptanceRecord;
use rule::{RuleText, WitnessStatus};

/// FULL (complete coverage of the sector, R33) or BOXED (user box, R35) node.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTarget {
    Full,
    Boxed { box_id: String },
}

/// What the chain needs to know about the node.
pub struct NodeView<'a> {
    pub node: String,
    pub frame: &'a NodeFrame,
    pub cmp: &'a NodeComparator,
    pub target: NodeTarget,
    /// Per-node spec value (U-12): `Replay` (complete nodes) or `PerKey` (boxed nodes).
    pub required: RequiredEvidence,
    /// Dropped-line masks of zero sectors (hand-downs that vanish).
    pub zero_sectors: Option<BTreeSet<u64>>,
    /// Declared terminals (preferred + other, R41 P-4): never dispatched, never served.
    pub terminals: BTreeSet<Key>,
}

impl NodeView<'_> {
    pub fn arity(&self) -> usize {
        self.frame.arity()
    }
    pub fn lines(&self) -> Vec<bool> {
        self.frame.mask().to_vec()
    }
}

/// The attempt that produced the candidate (ledgerfix: an accept cites its producing attempt).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Producer {
    pub job: String,
    pub attempt: u32,
    pub table_digest: String,
}

/// Where the candidate's record lives.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleSource {
    pub table_id: String,
    pub table_path: String,
    pub table_digest: String,
    /// 0-based position among the table's rule records.
    pub ordinal: u64,
    pub byte_offset: Option<u64>,
    pub producer: Option<Producer>,
}

/// Priority classes of the dispatch order (plan 2.4.7 D1), in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorityClass {
    Input,
    FaceOrLocus,
    Point,
    SymmetryCredit,
    Terminal,
}

/// A stated position in the dispatch order: sorted by (class, input priority, fixer round, item id, rule index)
/// (D1, D6).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Position {
    pub class: PriorityClass,
    pub input_priority: u32,
    pub round: u32,
    pub item: String,
    pub index: u32,
}

/// The candidate.
pub struct Candidate<'a> {
    pub rule: &'a RuleText,
    pub source: RuleSource,
    pub position: Position,
}

/// Failure kinds a key can have (the exclusion policy chooses which of them become exclusions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureKind {
    /// The candidate is the first selected rule but cannot be used (pole locus, recenter bound, elision).
    Blocked,
    Irregular,
    Hidden,
    NonMembership,
    NotDescending,
}

/// The policy of check 6.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct AcceptPolicy {
    /// Key failures that become exclusions; any other failure rejects the rule (default: plan 2.4.7 D7's list plus
    /// `Blocked`; `NotDescending` rejects, plan 2.4.4).
    pub exclude_on: BTreeSet<FailureKind>,
    /// Upper bound on prover rounds (each round removes at least one failed key).
    pub max_prover_rounds: usize,
}

impl Default for AcceptPolicy {
    fn default() -> Self {
        AcceptPolicy {
            exclude_on: [
                FailureKind::Blocked,
                FailureKind::Irregular,
                FailureKind::Hidden,
                FailureKind::NonMembership,
            ]
            .into_iter()
            .collect(),
            max_prover_rounds: 64,
        }
    }
}

/// The checks of the chain, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    Record,
    Served,
    Coverage,
    Regularity,
    Descent,
    Evidence,
    Exclusions,
    Scope,
}

/// Exclusion target: a key, or a locus (affine equations) for exclusions handed over from the ladder.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionTarget {
    Key(Key),
    Locus { equations: Vec<String> },
}

/// One exclusion of the effective domain (plan 2.4.7 D7 (ii)).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
pub struct Exclusion {
    pub target: ExclusionTarget,
    pub reason: FailureKind,
    /// The stored disproof / reason text (pole term, non-membership certificate, blocking verdict).
    pub disproof: String,
}

/// The effective domain = the rule's case minus its exclusions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct EffectiveDomain {
    pub rule_sha256: String,
    pub exclusions: Vec<Exclusion>,
}

/// What a rejection points at.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Witness {
    Structural(String),
    AtKey { key: Key, detail: String },
    Rule(String),
}

/// What an undecided chain is missing.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Missing {
    pub key: Option<Key>,
    pub what: String,
}

/// The verdict of the chain.
#[derive(Clone, Debug, PartialEq)]
pub enum AcceptVerdict {
    Accepted {
        position: Position,
        effective: EffectiveDomain,
        record: Box<AcceptanceRecord>,
        /// The evidence artefact (the caller writes `body` / cites the answer file).
        artefact: Option<EvidenceArtefact>,
    },
    Rejected {
        check: Check,
        witness: Witness,
    },
    Undecided {
        check: Check,
        missing: Vec<Missing>,
    },
    NeedsExternal {
        check: Check,
        requests: Vec<ExternalRequest>,
    },
}

impl AcceptVerdict {
    pub fn is_accepted(&self) -> bool {
        matches!(self, AcceptVerdict::Accepted { .. })
    }
    /// The check that decided a non-acceptance.
    pub fn check(&self) -> Option<Check> {
        match self {
            AcceptVerdict::Accepted { .. } => None,
            AcceptVerdict::Rejected { check, .. }
            | AcceptVerdict::Undecided { check, .. }
            | AcceptVerdict::NeedsExternal { check, .. } => Some(*check),
        }
    }
}

/// Caller mistakes (every mathematical outcome is a verdict).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AcceptError {
    Arity { expected: usize, got: usize },
    Cover(String),
    Position(String),
}

impl std::fmt::Display for AcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AcceptError::Arity { expected, got } => write!(f, "arity {got}, node arity {expected}"),
            AcceptError::Cover(e) => write!(f, "dispatch: {e}"),
            AcceptError::Position(e) => write!(f, "position: {e}"),
        }
    }
}
impl std::error::Error for AcceptError {}

/// Back-ends of one chain run: per-key facts (regularity + coefficient zero tests) and the evidence prover.
pub trait KeyFacts: Regularity + CoefficientFacts {}
impl<T: Regularity + CoefficientFacts> KeyFacts for T {}

/// Two fact sources combined: regularity from `R`, coefficients from `C` (e.g. EC-2 regularity, exact coefficients).
pub struct FactPair<R, C> {
    pub regularity: R,
    pub coefficients: C,
}
impl<R: Regularity, C> Regularity for FactPair<R, C> {
    fn regular(&mut self, rule: &RuleText, key: &[i32]) -> RegVerdict {
        self.regularity.regular(rule, key)
    }
}
impl<R, C: CoefficientFacts> CoefficientFacts for FactPair<R, C> {
    fn coefficient(&mut self, rule: &RuleText, key: &[i32], term: usize) -> crate::closure::facts::CoefFact {
        self.coefficients.coefficient(rule, key, term)
    }
}

/// Inputs of one chain run.
pub struct AcceptInput<'a> {
    pub node: NodeView<'a>,
    /// Tables in force placed BEFORE the candidate (None = nothing before it).
    pub before: Option<&'a Dispatcher>,
    /// Rules placed AFTER the candidate (winner diff of a non-final position); None = the candidate is last.
    pub after: Option<&'a Dispatcher>,
    pub candidate: Candidate<'a>,
    pub gap_points: &'a [Key],
    /// The working key range (box + envelope for boxed nodes; the checked range for FULL nodes).
    pub working: &'a [Key],
    pub policy: AcceptPolicy,
}

/// Where the composed dispatch (before, candidate, after) stops at a key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Declared terminal: not dispatched.
    Terminal,
    /// A rule before the candidate decides (any verdict, incl. open ones).
    Before(Dispatch),
    /// The candidate applies: served.
    Served,
    /// The candidate is selected but blocked / undecided.
    Stopped { undecided: bool, verdict: String },
    /// Neither: a rule after it (or nothing).
    After(Option<Dispatch>),
}

/// First match over the concatenation (before, candidate, after), exactly as `Dispatcher::first_match` over the
/// concatenated table list (sequential composition; nothing is copied).
pub fn composed_stop(
    node: &NodeView,
    before: Option<&Dispatcher>,
    rule: &RuleText,
    after: Option<&Dispatcher>,
    key: &[i32],
) -> Result<Stop, AcceptError> {
    composed_stop_domain(node, before, rule.domain.as_ref(), after, key)
}

/// [`composed_stop`] for a bare domain (recipe rules carry a domain and no record).
pub fn composed_stop_domain(
    node: &NodeView,
    before: Option<&Dispatcher>,
    domain: Option<&crate::domain::CaseDomain>,
    after: Option<&Dispatcher>,
    key: &[i32],
) -> Result<Stop, AcceptError> {
    if node.terminals.contains(key) {
        return Ok(Stop::Terminal);
    }
    if let Some(b) = before {
        let d = b.first_match(key).map_err(|e| AcceptError::Cover(e.to_string()))?;
        if d != Dispatch::Uncovered {
            return Ok(Stop::Before(d));
        }
    }
    let Some(dom) = domain else {
        return Ok(Stop::After(None));
    };
    let v = dom.applies(key);
    match v.stage() {
        Stage::Applies => Ok(Stop::Served),
        Stage::Blocked => Ok(Stop::Stopped {
            undecided: false,
            verdict: format!("{v:?}"),
        }),
        Stage::Undecided => Ok(Stop::Stopped {
            undecided: true,
            verdict: format!("{v:?}"),
        }),
        Stage::NotSelected => match after {
            None => Ok(Stop::After(None)),
            Some(a) => Ok(Stop::After(Some(
                a.first_match(key).map_err(|e| AcceptError::Cover(e.to_string()))?,
            ))),
        },
    }
}

/// The served / stopped / displaced sets of check 1 (also used by the winner diff after an assembly change).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServedSets {
    pub served: Vec<Key>,
    /// Keys where the candidate stops dispatch without serving (verdict text; true = undecided).
    pub stopped: Vec<(Key, bool, String)>,
    /// Keys a rule placed after the candidate served before (now the candidate's).
    pub displaced: Vec<Key>,
    pub terminals_in_range: usize,
    pub working: usize,
}

/// Check 1 over a working range.
pub fn served_sets(
    node: &NodeView,
    before: Option<&Dispatcher>,
    rule: &RuleText,
    after: Option<&Dispatcher>,
    working: &[Key],
) -> Result<ServedSets, AcceptError> {
    let mut seen = BTreeSet::new();
    let mut out = ServedSets::default();
    for k in working {
        if k.len() != node.arity() {
            return Err(AcceptError::Arity {
                expected: node.arity(),
                got: k.len(),
            });
        }
        if !seen.insert(k.clone()) {
            continue;
        }
        out.working += 1;
        let stop = composed_stop(node, before, rule, after, k)?;
        let took_from_after = |out: &mut ServedSets| {
            if let Some(a) = after
                && let Ok(Dispatch::Covered(_)) = a.first_match(k)
            {
                out.displaced.push(k.clone());
            }
        };
        match stop {
            Stop::Terminal => out.terminals_in_range += 1,
            Stop::Served => {
                took_from_after(&mut out);
                out.served.push(k.clone());
            }
            Stop::Stopped { undecided, verdict } => {
                took_from_after(&mut out);
                out.stopped.push((k.clone(), undecided, verdict));
            }
            Stop::Before(_) | Stop::After(_) => {}
        }
    }
    Ok(out)
}

fn describe_stop(s: &Stop) -> String {
    match s {
        Stop::Terminal => "declared terminal".into(),
        Stop::Before(d) => format!("decided before the candidate: {d:?}"),
        Stop::Served => "served".into(),
        Stop::Stopped { verdict, .. } => format!("candidate selected but not usable: {verdict}"),
        Stop::After(None) => "not in the candidate's domain".into(),
        Stop::After(Some(d)) => format!("not in the candidate's domain; after it: {d:?}"),
    }
}

/// Per-key bookkeeping of the chain.
#[derive(Default)]
struct KeyLedger {
    failed: BTreeMap<Key, (FailureKind, Check, String)>,
    unknown: BTreeMap<Key, (Check, String)>,
}

impl KeyLedger {
    fn fail(&mut self, k: &Key, kind: FailureKind, check: Check, detail: String) {
        self.failed.entry(k.clone()).or_insert((kind, check, detail));
        self.unknown.remove(k);
    }
    fn unknown(&mut self, k: &Key, check: Check, why: String) {
        if !self.failed.contains_key(k) {
            self.unknown.entry(k.clone()).or_insert((check, why));
        }
    }
    fn open(&self, k: &Key) -> bool {
        !self.failed.contains_key(k) && !self.unknown.contains_key(k)
    }
}

/// Outcome summaries of the checks (for the record).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct CheckOutcome {
    pub check: Option<Check>,
    pub outcome: String,
    pub counts: BTreeMap<String, u64>,
}

fn outcome(check: Check, text: &str, counts: &[(&str, u64)]) -> CheckOutcome {
    CheckOutcome {
        check: Some(check),
        outcome: text.to_string(),
        counts: counts.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
    }
}

/// The scope a record certifies (check 7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// FULL node: the symbolic conditions hold on the whole effective domain.
    FullDomain,
    /// Only the working range (digest) is certified; `missing` = why not the whole domain (FULL nodes).
    KeyRange { digest: String, missing: Vec<String> },
}

/// Run the acceptance chain (interface section 2).
pub fn accept_candidate(
    input: &AcceptInput,
    facts: &mut dyn KeyFacts,
    prover: &mut dyn EvidenceProver,
) -> Result<AcceptVerdict, AcceptError> {
    let node = &input.node;
    let rule = input.candidate.rule;
    let n = node.arity();
    let mut checks: Vec<CheckOutcome> = Vec::new();

    // ---- 0 Record (A0)
    if rule.record.fixed.len() != n {
        return Err(AcceptError::Arity {
            expected: n,
            got: rule.record.fixed.len(),
        });
    }
    let pos = &input.candidate.position;
    match pos.class {
        PriorityClass::Point if !rule.is_point() => {
            return Err(AcceptError::Position("class Point for a rule with free slots".into()));
        }
        PriorityClass::FaceOrLocus if rule.is_point() => {
            return Err(AcceptError::Position("class FaceOrLocus for a point rule".into()));
        }
        PriorityClass::SymmetryCredit | PriorityClass::Terminal => {
            return Err(AcceptError::Position("a rule is never placed in the credit / terminal class".into()));
        }
        _ => {}
    }
    let rejected = |check, w| Ok(AcceptVerdict::Rejected { check, witness: w });
    if let Some(e) = &rule.domain_error {
        return rejected(Check::Record, Witness::Structural(format!("domain: {e}")));
    }
    if rule.empty_exception_branch {
        return rejected(Check::Record, Witness::Structural("empty exception AND-list".into()));
    }
    match rule.witness_status() {
        WitnessStatus::Elided => {
            return rejected(Check::Record, Witness::Structural("witness loci elided (R24 (6))".into()));
        }
        WitnessStatus::Absent => {
            return Ok(AcceptVerdict::Undecided {
                check: Check::Record,
                missing: vec![Missing {
                    key: None,
                    what: "witness loci absent (incomplete record: re-lift)".into(),
                }],
            });
        }
        WitnessStatus::Present { .. } => {}
    }
    if rule.guard_version.unwrap_or(0) < 2 {
        return Ok(AcceptVerdict::Undecided {
            check: Check::Record,
            missing: vec![Missing {
                key: None,
                what: format!("guard version {:?} < 2: d2 unknown until re-lifted", rule.guard_version),
            }],
        });
    }
    checks.push(outcome(Check::Record, "pass", &[]));

    // ---- 1 Served (winner diff over the working range)
    let sets = served_sets(node, input.before, rule, input.after, input.working)?;
    let mut shadow = None;
    if node.target == NodeTarget::Full
        && let (Some(b), Some(dom)) = (input.before, &rule.domain)
    {
        let mut others = Vec::new();
        for &t in b.priority() {
            others.extend(b.tables()[t].domains.iter());
        }
        let c = contained_in(dom, &others, node.frame);
        if let Containment::Proved { certificate } = &c {
            return rejected(
                Check::Served,
                Witness::Rule(format!("shadowed on the whole domain by the tables in force: {certificate}")),
            );
        }
        shadow = Some(c);
    }
    if sets.served.is_empty() {
        return rejected(
            Check::Served,
            Witness::Structural(format!("wins no key of the working range ({} keys)", sets.working)),
        );
    }
    checks.push(outcome(
        Check::Served,
        "pass",
        &[
            ("working", sets.working as u64),
            ("served", sets.served.len() as u64),
            ("stopped", sets.stopped.len() as u64),
            ("displaced", sets.displaced.len() as u64),
            ("terminals_in_range", sets.terminals_in_range as u64),
        ],
    ));
    let mut led = KeyLedger::default();
    for (k, undecided, v) in &sets.stopped {
        if *undecided {
            led.unknown(k, Check::Served, v.clone());
        } else {
            led.fail(k, FailureKind::Blocked, Check::Served, v.clone());
        }
    }

    // ---- 2 Coverage at the gap points (credit names the rule, R8)
    let served_set: BTreeSet<&Key> = sets.served.iter().collect();
    let mut not_credited: Vec<(Key, String)> = Vec::new();
    let mut credited: Vec<Key> = Vec::new();
    for g in input.gap_points {
        if served_set.contains(g) {
            credited.push(g.clone());
        } else {
            let s = composed_stop(node, input.before, rule, input.after, g)?;
            not_credited.push((g.clone(), describe_stop(&s)));
        }
    }
    if credited.is_empty() {
        return rejected(
            Check::Coverage,
            Witness::Structural(format!("serves none of the {} gap points", input.gap_points.len())),
        );
    }
    checks.push(outcome(
        Check::Coverage,
        "pass",
        &[("credited", credited.len() as u64), ("not_credited", not_credited.len() as u64)],
    ));

    // ---- 3 Regularity at every served key (exact, per key, R38)
    for k in &sets.served {
        match facts.regular(rule, k) {
            RegVerdict::Regular => {}
            RegVerdict::Pole { term } => led.fail(k, FailureKind::Irregular, Check::Regularity, format!("pole of term {term}")),
            RegVerdict::Hidden { locus } => led.fail(k, FailureKind::Hidden, Check::Regularity, format!("witness locus {locus} vanishes")),
            RegVerdict::Unknown(w) => led.unknown(k, Check::Regularity, w),
        }
    }
    checks.push(outcome(Check::Regularity, "done", &[("failed", led.failed.len() as u64), ("unknown", led.unknown.len() as u64)]));

    // ---- 4 Descent at every served key (comparator with tie-breaks; coefficient zero verified)
    let inst = match Instantiator::new(rule) {
        Ok(i) => i,
        Err(e) => return rejected(Check::Descent, Witness::Structural(format!("rhs: {e}"))),
    };
    let sectors = Sectors {
        lines: node.lines(),
        zero_sectors: node.zero_sectors.clone(),
    };
    let mut dropped_zero = 0u64;
    for k in &sets.served {
        if !led.open(k) {
            continue;
        }
        match descent_at(&inst, &sectors, node.cmp, facts, rule, k) {
            DescentAt::Pass { dropped_zero: z, .. } => dropped_zero += z as u64,
            DescentAt::Fail(f) => {
                let d = match f {
                    DescentFailure::NotLower { term, rhs } => format!("term {term} rhs {rhs:?} not lower, coefficient nonzero"),
                    DescentFailure::PositiveNumeratorNonZero { term, rhs } => {
                        format!("term {term} rhs {rhs:?} positive numerator, coefficient nonzero")
                    }
                };
                led.fail(k, FailureKind::NotDescending, Check::Descent, d)
            }
            DescentAt::Unknown { term, rhs, why } => led.unknown(k, Check::Descent, format!("term {term} rhs {rhs:?}: {why}")),
        }
    }
    checks.push(outcome(
        Check::Descent,
        "done",
        &[("failed", led.failed.len() as u64), ("unknown", led.unknown.len() as u64), ("dropped_zero", dropped_zero)],
    ));

    // Non-excludable failures reject; unknowns make the chain undecided BEFORE any evidence is spent.
    if let Some(v) = reject_or_undecide(&led, &input.policy) {
        return Ok(v);
    }

    // ---- 5 Evidence at the required level, for EXACTLY the served set
    if !node.required.satisfied_by(prover.level()) {
        return Ok(AcceptVerdict::Undecided {
            check: Check::Evidence,
            missing: vec![Missing {
                key: None,
                what: format!(
                    "the prover gives {:?}, the node requires {:?}",
                    prover.level(),
                    node.required
                ),
            }],
        });
    }
    let mut artefact: Option<EvidenceArtefact> = None;
    let mut rule_failure: Option<prover::RuleLevel> = None;
    let mut rounds = 0;
    loop {
        let keys: Vec<Key> = sets.served.iter().filter(|k| led.open(k)).cloned().collect();
        if keys.is_empty() {
            break;
        }
        rounds += 1;
        if rounds > input.policy.max_prover_rounds {
            return Ok(AcceptVerdict::Undecided {
                check: Check::Evidence,
                missing: vec![Missing {
                    key: None,
                    what: "prover rounds exhausted".into(),
                }],
            });
        }
        match prover.prove(rule, &input.candidate.source, &keys) {
            ProverAnswer::NeedsExternal(requests) => {
                return Ok(AcceptVerdict::NeedsExternal {
                    check: Check::Evidence,
                    requests,
                });
            }
            ProverAnswer::Unavailable(why) => {
                return Ok(AcceptVerdict::Undecided {
                    check: Check::Evidence,
                    missing: vec![Missing {
                        key: None,
                        what: format!("prover unavailable: {why}"),
                    }],
                });
            }
            ProverAnswer::PerKey {
                keys: statuses,
                artefact: art,
                rule_level,
            } => {
                let answered: BTreeMap<&Key, &KeyStatus> = statuses.iter().map(|(k, s)| (k, s)).collect();
                let mut new_failures = false;
                for k in &keys {
                    match answered.get(k) {
                        Some(KeyStatus::Reached(l)) if node.required.satisfied_by(*l) => {}
                        Some(KeyStatus::Reached(l)) => led.unknown(k, Check::Evidence, format!("reached {l:?} only")),
                        Some(KeyStatus::Failed(f)) => {
                            new_failures = true;
                            let (kind, d) = match f {
                                KeyFailure::NonMembership { disproof } => (FailureKind::NonMembership, disproof.clone()),
                                KeyFailure::Irregular { detail } => (FailureKind::Irregular, detail.clone()),
                                KeyFailure::Hidden { detail } => (FailureKind::Hidden, detail.clone()),
                            };
                            led.fail(k, kind, Check::Evidence, d);
                        }
                        Some(KeyStatus::NotCredited(w)) => {
                            led.unknown(k, Check::Evidence, format!("prover: {w}, d1 says applies"))
                        }
                        Some(KeyStatus::Unsupported(w)) => led.unknown(k, Check::Evidence, format!("unsupported: {w}")),
                        Some(KeyStatus::Unknown(w)) => led.unknown(k, Check::Evidence, w.clone()),
                        None => led.unknown(k, Check::Evidence, "the prover did not answer this key".into()),
                    }
                }
                if let Some(r) = &rule_level {
                    if r.disproof {
                        return rejected(Check::Evidence, Witness::Rule(r.text.clone()));
                    }
                    // a rule-level failure: no key reaches the level (statuses say Unknown / Failed per key)
                    rule_failure = Some(r.clone());
                }
                artefact = art;
                if !new_failures {
                    break;
                }
                // keys failed: the artefact must cover EXACTLY the remaining served set: ask again
            }
        }
    }
    let n_reached = sets.served.iter().filter(|k| led.open(k)).count() as u64;
    if let Some(r) = &rule_failure
        && n_reached > 0
    {
        // a rule-level failure can not coexist with reached keys: fail closed
        return Ok(AcceptVerdict::Undecided {
            check: Check::Evidence,
            missing: vec![Missing {
                key: None,
                what: format!("rule-level evidence failure: {}", r.text),
            }],
        });
    }
    checks.push(outcome(
        Check::Evidence,
        "done",
        &[("reached", n_reached), ("failed", led.failed.len() as u64), ("unknown", led.unknown.len() as u64), ("rounds", rounds as u64)],
    ));

    // ---- 6 Exclusions
    if let Some(v) = reject_or_undecide(&led, &input.policy) {
        return Ok(v);
    }
    let final_served: Vec<Key> = sets.served.iter().filter(|k| led.open(k)).cloned().collect();
    let credited_final: Vec<Key> = credited.iter().filter(|k| led.open(k)).cloned().collect();
    for k in credited.iter().filter(|k| !led.open(k)) {
        let (_, _, d) = &led.failed[k];
        not_credited.push((k.clone(), format!("excluded: {d}")));
    }
    if credited_final.is_empty() {
        let (k, (kind, check, d)) = led
            .failed
            .iter()
            .find(|(k, _)| credited.contains(k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .unwrap_or((credited[0].clone(), (FailureKind::Blocked, Check::Exclusions, String::new())));
        return rejected(check, Witness::AtKey { key: k, detail: format!("{kind:?}: {d}; no gap point left") });
    }
    let art = match artefact {
        Some(a) if a.keys_digest == prover::keys_digest(n, &final_served) => a,
        _ => {
            return Ok(AcceptVerdict::Undecided {
                check: Check::Evidence,
                missing: vec![Missing {
                    key: None,
                    what: "no evidence artefact for exactly the served set".into(),
                }],
            });
        }
    };
    let exclusions: Vec<Exclusion> = led
        .failed
        .iter()
        .map(|(k, (kind, _, d))| Exclusion {
            target: ExclusionTarget::Key(k.clone()),
            reason: *kind,
            disproof: d.clone(),
        })
        .collect();
    checks.push(outcome(Check::Exclusions, "pass", &[("exclusions", exclusions.len() as u64)]));

    // ---- 7 Scope
    let working_digest = prover::keys_digest(n, input.working);
    let not_proved = symbolic_not_proved(rule, node.cmp, &node.lines());
    let scope = match node.target {
        NodeTarget::Boxed { .. } => Scope::KeyRange {
            digest: working_digest.clone(),
            missing: vec![],
        },
        NodeTarget::Full => {
            let mut missing = Vec::new();
            if !exclusions.is_empty() {
                missing.push(format!("{} exclusions", exclusions.len()));
            }
            if art.schema != crate::ibp_rows::REPLAY_SCHEMA {
                missing.push("identity not tested at generic points (per-key artefact only)".into());
            }
            if !not_proved.is_empty() {
                missing.push(format!("symbolic descent not proved for terms {not_proved:?}"));
            }
            match &shadow {
                Some(Containment::NotContained { .. }) => {}
                Some(Containment::Unknown { reason }) => missing.push(format!("shadowing undecided: {reason}")),
                _ => missing.push("shadowing not decided".into()),
            }
            if missing.is_empty() {
                Scope::FullDomain
            } else {
                Scope::KeyRange {
                    digest: working_digest.clone(),
                    missing,
                }
            }
        }
    };
    checks.push(outcome(Check::Scope, if scope == Scope::FullDomain { "full_domain" } else { "key_range" }, &[]));

    let effective = EffectiveDomain {
        rule_sha256: rule.line_sha256.clone(),
        exclusions: exclusions.clone(),
    };
    let record = AcceptanceRecord::build(record::RecordParts {
        node,
        candidate: &input.candidate,
        policy: &input.policy,
        working_digest,
        working_count: sets.working as u64,
        served: &final_served,
        stopped: sets.stopped.len() as u64,
        displaced: sets.displaced.len() as u64,
        gap_count: input.gap_points.len() as u64,
        credited: &credited_final,
        not_credited: &not_credited,
        checks: &checks,
        exclusions: &exclusions,
        evidence_level: node.required.as_evidence(),
        artefact: &art,
        scope: &scope,
        shadowed: shadow.as_ref().map(|c| format!("{c:?}")),
        pattern_not_proved: not_proved.iter().copied().collect(),
    });
    Ok(AcceptVerdict::Accepted {
        position: input.candidate.position.clone(),
        effective,
        record: Box::new(record),
        artefact: Some(art),
    })
}

fn reject_or_undecide(led: &KeyLedger, policy: &AcceptPolicy) -> Option<AcceptVerdict> {
    let mut worst: Option<(&Key, &(FailureKind, Check, String))> = None;
    for (k, v) in &led.failed {
        if !policy.exclude_on.contains(&v.0) && worst.is_none_or(|(_, w)| v.1 < w.1) {
            worst = Some((k, v));
        }
    }
    if let Some((k, (kind, check, d))) = worst {
        return Some(AcceptVerdict::Rejected {
            check: *check,
            witness: Witness::AtKey {
                key: k.clone(),
                detail: format!("{kind:?}: {d}"),
            },
        });
    }
    if !led.unknown.is_empty() {
        let check = led.unknown.values().map(|(c, _)| *c).min().unwrap_or(Check::Evidence);
        return Some(AcceptVerdict::Undecided {
            check,
            missing: led
                .unknown
                .iter()
                .map(|(k, (c, w))| Missing {
                    key: Some(k.clone()),
                    what: format!("{c:?}: {w}"),
                })
                .collect(),
        });
    }
    None
}
