//! State machines of the campaign and the reducer that computes the campaign state from the ledger.
//!
//! * Every machine is a pure function `apply(state, event[, context]) -> Result<state, IllegalTransition>` with
//!   an exhaustive `match` and a transition table (`*_TABLE`) that the tests check against `apply`.
//! * [`reduce`] / [`CampaignState::apply_record`] is the ONLY way campaign state is computed: the supervisor
//!   keeps no truth outside the ledger; the in-memory state of a [`crate::ledger::Ledger`] is a cache of the
//!   reduction of its records.
//! * Replay is idempotent: every record kind has a replay rule (see the interface document, table "replay
//!   rules"): exact repeats are no-ops, job events carry the attempt number and older attempts are ignored,
//!   intents and dones are keyed by operation id.
//!
//! The classification of a finished attempt comes from the child-job contract (interfaces/child-job.md 6.5).
//! The contract crate (`rustred-campaign-contract`, sibling package) had no types when this was written, so
//! [`FinishClass`] is a LOCAL MIRROR of its outcome kinds; [`FinishClass::from_contract_kind`] is the single
//! conversion point to replace by the crate's enum when it lands.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ledger::{Event, FileRef, Record};

// ------------------------------------------------------------------------------------------------------------
// Common vocabulary
// ------------------------------------------------------------------------------------------------------------

/// A ladder rung `L0`..`L9` (design note auto-closure.md 1(c): L0 generic face case ... L5 terminal).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Rung(pub u8);

impl Serialize for Rung {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format!("L{}", self.0))
    }
}

impl<'de> Deserialize<'de> for Rung {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.strip_prefix('L')
            .and_then(|n| n.parse::<u8>().ok())
            .map(Rung)
            .ok_or_else(|| serde::de::Error::custom(format!("bad rung {s:?} (expected L<n>)")))
    }
}

impl std::fmt::Display for Rung {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "L{}", self.0)
    }
}

/// Evidence level of an accepted rule (checker comment A6): geometry alone, an independent identity test, an
/// exact identity proof. Ordered: a level only ever increases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    GeometryAccepted,
    IdentityTested,
    IdentityProved,
}

impl EvidenceLevel {
    pub const ALL: [EvidenceLevel; 3] =
        [EvidenceLevel::GeometryAccepted, EvidenceLevel::IdentityTested, EvidenceLevel::IdentityProved];
    pub fn word(self) -> &'static str {
        match self {
            EvidenceLevel::GeometryAccepted => "geometry_accepted",
            EvidenceLevel::IdentityTested => "identity_tested",
            EvidenceLevel::IdentityProved => "identity_proved",
        }
    }
}

/// A refused transition: the record is not applied (and, on the writing side, never written).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IllegalTransition {
    pub machine: &'static str,
    pub from: String,
    pub event: String,
    pub why: String,
}

impl IllegalTransition {
    fn new(machine: &'static str, from: impl Into<String>, event: impl Into<String>, why: impl Into<String>) -> Self {
        IllegalTransition { machine, from: from.into(), event: event.into(), why: why.into() }
    }
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "illegal {} transition: {} --{}--> ({})", self.machine, self.from, self.event, self.why)
    }
}

impl std::error::Error for IllegalTransition {}

// ------------------------------------------------------------------------------------------------------------
// (i) JOB ATTEMPT
// ------------------------------------------------------------------------------------------------------------

/// Outcome kinds of a finished attempt: LOCAL MIRROR of the contract's classification (child-job.md 6.5,
/// PLAN.v2 3.3.4 closed list plus the checker verdicts).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishClass {
    Complete,
    RuleFound,
    NoRule,
    DecidedNegative,
    Undecided,
    Spill,
    KilledTime,
    KilledMemory,
    Interrupted,
    UsageError,
    RefusedInput,
    RefusedContract,
    Lost,
    Crashed,
}

impl FinishClass {
    pub const ALL: [FinishClass; 14] = [
        FinishClass::Complete,
        FinishClass::RuleFound,
        FinishClass::NoRule,
        FinishClass::DecidedNegative,
        FinishClass::Undecided,
        FinishClass::Spill,
        FinishClass::KilledTime,
        FinishClass::KilledMemory,
        FinishClass::Interrupted,
        FinishClass::UsageError,
        FinishClass::RefusedInput,
        FinishClass::RefusedContract,
        FinishClass::Lost,
        FinishClass::Crashed,
    ];

    /// The ONE conversion point from the contract's words (child-job.md 6.5 "kind" column; refused (input) =
    /// `refused_input`, refused (contract) = `refused_contract`). Replace by the contract crate's enum.
    pub fn from_contract_kind(kind: &str) -> Option<FinishClass> {
        Some(match kind {
            "complete" => FinishClass::Complete,
            "rule_found" => FinishClass::RuleFound,
            "no_rule" => FinishClass::NoRule,
            "decided_negative" => FinishClass::DecidedNegative,
            "undecided" => FinishClass::Undecided,
            "spill" => FinishClass::Spill,
            "killed_time" => FinishClass::KilledTime,
            "killed_memory" => FinishClass::KilledMemory,
            "interrupted" => FinishClass::Interrupted,
            "usage_error" => FinishClass::UsageError,
            "refused_input" | "refused" => FinishClass::RefusedInput,
            "refused_contract" => FinishClass::RefusedContract,
            "lost" => FinishClass::Lost,
            "crashed" => FinishClass::Crashed,
            _ => return None,
        })
    }

    /// Terminal attempt state reached by this class.
    pub fn target(self) -> AttemptTarget {
        use FinishClass::*;
        match self {
            Complete | RuleFound | NoRule | DecidedNegative | Undecided => AttemptTarget::SucceededSealed,
            Spill | KilledTime | KilledMemory => AttemptTarget::BudgetStopped,
            Interrupted => AttemptTarget::Interrupted,
            Lost => AttemptTarget::Lost,
            UsageError | RefusedInput | RefusedContract | Crashed => AttemptTarget::Failed,
        }
    }

    pub fn word(self) -> &'static str {
        use FinishClass::*;
        match self {
            Complete => "complete",
            RuleFound => "rule_found",
            NoRule => "no_rule",
            DecidedNegative => "decided_negative",
            Undecided => "undecided",
            Spill => "spill",
            KilledTime => "killed_time",
            KilledMemory => "killed_memory",
            Interrupted => "interrupted",
            UsageError => "usage_error",
            RefusedInput => "refused_input",
            RefusedContract => "refused_contract",
            Lost => "lost",
            Crashed => "crashed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptTarget {
    SucceededSealed,
    BudgetStopped,
    Interrupted,
    Lost,
    Failed,
}

/// State of the CURRENT attempt of a job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", content = "class", rename_all = "snake_case")]
pub enum AttemptState {
    Queued,
    Launching,
    Running,
    SucceededSealed(FinishClass),
    Failed(FinishClass),
    BudgetStopped(FinishClass),
    Interrupted,
    Lost,
    Cancelled,
}

impl AttemptState {
    pub fn tag(&self) -> &'static str {
        match self {
            AttemptState::Queued => "queued",
            AttemptState::Launching => "launching",
            AttemptState::Running => "running",
            AttemptState::SucceededSealed(_) => "succeeded_sealed",
            AttemptState::Failed(_) => "failed",
            AttemptState::BudgetStopped(_) => "budget_stopped",
            AttemptState::Interrupted => "interrupted",
            AttemptState::Lost => "lost",
            AttemptState::Cancelled => "cancelled",
        }
    }
    pub const TAGS: [&'static str; 9] = [
        "queued",
        "launching",
        "running",
        "succeeded_sealed",
        "failed",
        "budget_stopped",
        "interrupted",
        "lost",
        "cancelled",
    ];
    /// True when no process can exist for this attempt any more.
    pub fn is_settled(&self) -> bool {
        !matches!(self, AttemptState::Launching | AttemptState::Running)
    }
}

/// Attempt-level events (payload-free view used by the machine; the ledger's [`JobEvent`] carries the data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptEvent {
    Launch,
    Started,
    Finished { class: FinishClass, sealed: bool },
    Interrupt,
    Lose,
    /// New attempt after `interrupted` / `lost` (campaign resume).
    Requeue,
    /// New attempt after `failed` (retry policy, e.g. crashed once).
    Retry,
    Cancel,
}

impl AttemptEvent {
    pub fn tag(&self) -> &'static str {
        match self {
            AttemptEvent::Launch => "launch",
            AttemptEvent::Started => "started",
            AttemptEvent::Finished { .. } => "finished",
            AttemptEvent::Interrupt => "interrupt",
            AttemptEvent::Lose => "lose",
            AttemptEvent::Requeue => "requeue",
            AttemptEvent::Retry => "retry",
            AttemptEvent::Cancel => "cancel",
        }
    }
}

/// Transition table of the attempt machine: (from, event, to). `finished:*` expands to the class target.
/// Exact repeats (same event re-applied in the state it produced) are no-ops and not listed.
pub const ATTEMPT_TABLE: &[(&str, &str, &str)] = &[
    ("queued", "launch", "launching"),
    ("queued", "cancel", "cancelled"),
    ("launching", "started", "running"),
    ("launching", "finished", "<class target>"),
    ("launching", "interrupt", "interrupted"),
    ("launching", "lose", "lost"),
    ("running", "finished", "<class target>"),
    ("running", "interrupt", "interrupted"),
    ("running", "lose", "lost"),
    ("interrupted", "requeue", "queued (attempt + 1)"),
    ("lost", "requeue", "queued (attempt + 1)"),
    ("failed", "retry", "queued (attempt + 1)"),
];

/// The attempt machine. Returns the new state; `Requeue` / `Retry` return `Queued` (the caller increments the
/// attempt number). A success class without a sealed output is refused (acceptance needs a sealed table).
pub fn apply_attempt(state: AttemptState, ev: AttemptEvent) -> Result<AttemptState, IllegalTransition> {
    use AttemptEvent as E;
    use AttemptState as S;
    let illegal = |why: &str| Err(IllegalTransition::new("job attempt", state.tag(), ev.tag(), why));
    let finish = |class: FinishClass, sealed: bool| -> Result<AttemptState, IllegalTransition> {
        match class.target() {
            AttemptTarget::SucceededSealed if !sealed => Err(IllegalTransition::new(
                "job attempt",
                state.tag(),
                "finished",
                format!("class {} needs a sealed output", class.word()),
            )),
            AttemptTarget::SucceededSealed => Ok(S::SucceededSealed(class)),
            AttemptTarget::BudgetStopped => Ok(S::BudgetStopped(class)),
            AttemptTarget::Interrupted => Ok(S::Interrupted),
            AttemptTarget::Lost => Ok(S::Lost),
            AttemptTarget::Failed => Ok(S::Failed(class)),
        }
    };
    match (state, ev) {
        (S::Queued, E::Launch) => Ok(S::Launching),
        (S::Queued, E::Cancel) => Ok(S::Cancelled),
        (S::Queued, E::Requeue | E::Retry) => Ok(S::Queued), // repeat of the record that created this attempt
        (S::Queued, _) => illegal("attempt not launched"),
        (S::Launching, E::Launch) => Ok(S::Launching),
        (S::Launching, E::Started) => Ok(S::Running),
        (S::Launching, E::Finished { class, sealed }) => finish(class, sealed),
        (S::Launching, E::Interrupt) => Ok(S::Interrupted),
        (S::Launching, E::Lose) => Ok(S::Lost),
        (S::Launching, _) => illegal("attempt is launching"),
        (S::Running, E::Started) => Ok(S::Running),
        (S::Running, E::Finished { class, sealed }) => finish(class, sealed),
        (S::Running, E::Interrupt) => Ok(S::Interrupted),
        (S::Running, E::Lose) => Ok(S::Lost),
        (S::Running, _) => illegal("attempt is running"),
        (S::SucceededSealed(c), E::Finished { class, .. }) if c == class => Ok(state),
        (S::SucceededSealed(_), _) => illegal("attempt succeeded (final)"),
        (S::BudgetStopped(c), E::Finished { class, .. }) if c == class => Ok(state),
        (S::BudgetStopped(_), _) => illegal("attempt stopped by budget (final; the item escalates)"),
        (S::Failed(c), E::Finished { class, .. }) if c == class => Ok(state),
        (S::Failed(_), E::Retry) => Ok(S::Queued),
        (S::Failed(_), _) => illegal("attempt failed (only retry)"),
        (S::Interrupted, E::Interrupt | E::Finished { class: FinishClass::Interrupted, .. }) => Ok(state),
        (S::Interrupted, E::Requeue) => Ok(S::Queued),
        (S::Interrupted, _) => illegal("attempt interrupted (only requeue)"),
        (S::Lost, E::Lose | E::Finished { class: FinishClass::Lost, .. }) => Ok(state),
        (S::Lost, E::Requeue) => Ok(S::Queued),
        (S::Lost, _) => illegal("attempt lost (only requeue)"),
        (S::Cancelled, E::Cancel) => Ok(state),
        (S::Cancelled, _) => illegal("job cancelled (final)"),
    }
}

// ------------------------------------------------------------------------------------------------------------
// (ii) WORK ITEM
// ------------------------------------------------------------------------------------------------------------

/// Item phase. An item is one (face / locus / point set / phase-1 node ...) at ONE rung; escalation ends the
/// item and the supervisor creates the next-rung item with `parent` = this one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", content = "detail", rename_all = "snake_case")]
pub enum ItemPhase {
    Open,
    InProgress,
    Produced,
    Accepted(EvidenceLevel),
    Rejected(String),
    Escalated(Rung),
    TerminalCandidate,
    Terminal,
    Stale,
}

impl ItemPhase {
    pub fn tag(&self) -> &'static str {
        match self {
            ItemPhase::Open => "open",
            ItemPhase::InProgress => "in_progress",
            ItemPhase::Produced => "produced",
            ItemPhase::Accepted(_) => "accepted",
            ItemPhase::Rejected(_) => "rejected",
            ItemPhase::Escalated(_) => "escalated",
            ItemPhase::TerminalCandidate => "terminal_candidate",
            ItemPhase::Terminal => "terminal",
            ItemPhase::Stale => "stale",
        }
    }
    pub const TAGS: [&'static str; 9] = [
        "open",
        "in_progress",
        "produced",
        "accepted",
        "rejected",
        "escalated",
        "terminal_candidate",
        "terminal",
        "stale",
    ];
    pub fn index(&self) -> usize {
        Self::TAGS.iter().position(|t| *t == self.tag()).unwrap()
    }
    /// Open work: everything that is not accepted, escalated (superseded by its child) or terminal.
    pub fn is_open(&self) -> bool {
        !matches!(self, ItemPhase::Accepted(_) | ItemPhase::Escalated(_) | ItemPhase::Terminal)
    }
}

/// Item events (payload-free view; the ledger's [`ItemEvent`] carries the data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemMachineEvent {
    Start,
    Produce,
    Accept(EvidenceLevel),
    Upgrade(EvidenceLevel),
    Reject(String),
    Escalate(Rung),
    Requeue,
    ProposeTerminal,
    DeclareTerminal,
    MarkStale,
    Reopen,
}

impl ItemMachineEvent {
    pub fn tag(&self) -> &'static str {
        match self {
            ItemMachineEvent::Start => "start",
            ItemMachineEvent::Produce => "produce",
            ItemMachineEvent::Accept(_) => "accept",
            ItemMachineEvent::Upgrade(_) => "upgrade",
            ItemMachineEvent::Reject(_) => "reject",
            ItemMachineEvent::Escalate(_) => "escalate",
            ItemMachineEvent::Requeue => "requeue",
            ItemMachineEvent::ProposeTerminal => "propose_terminal",
            ItemMachineEvent::DeclareTerminal => "declare_terminal",
            ItemMachineEvent::MarkStale => "mark_stale",
            ItemMachineEvent::Reopen => "reopen",
        }
    }
}

/// Item state seen by the machine: phase and the rung the item lives on (for the escalation guard).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemState {
    pub phase: ItemPhase,
    pub rung: Rung,
}

pub const ITEM_TABLE: &[(&str, &str, &str)] = &[
    ("open", "start", "in_progress"),
    ("open", "propose_terminal", "terminal_candidate"),
    ("in_progress", "produce", "produced"),
    ("in_progress", "escalate", "escalated  [guard: to > rung]"),
    ("in_progress", "requeue", "open"),
    ("in_progress", "propose_terminal", "terminal_candidate"),
    ("produced", "accept", "accepted(level)"),
    ("produced", "reject", "rejected(reason)"),
    ("accepted", "upgrade", "accepted(higher level)  [guard: level > current]"),
    ("accepted", "reject", "rejected(reason)  (identity test or proof failed after geometry acceptance)"),
    ("rejected", "escalate", "escalated  [guard: to > rung]"),
    ("rejected", "requeue", "open"),
    ("rejected", "propose_terminal", "terminal_candidate"),
    ("terminal_candidate", "declare_terminal", "terminal  [guard: node bound, checked by the reducer]"),
    ("terminal_candidate", "reopen", "open"),
    ("<any but stale>", "mark_stale", "stale"),
    ("stale", "reopen", "open"),
];

pub fn apply_item(state: &ItemState, ev: &ItemMachineEvent) -> Result<ItemState, IllegalTransition> {
    use ItemMachineEvent as E;
    use ItemPhase as P;
    let illegal = |why: &str| Err(IllegalTransition::new("work item", state.phase.tag(), ev.tag(), why));
    let to = |phase: ItemPhase| Ok(ItemState { phase, rung: state.rung });
    let same = || Ok(state.clone());
    match (&state.phase, ev) {
        (_, E::MarkStale) => to(P::Stale), // from any phase; repeat at stale is a no-op
        (P::Open, E::Start) => to(P::InProgress),
        (P::Open, E::ProposeTerminal) => to(P::TerminalCandidate),
        (P::Open, E::Requeue | E::Reopen) => same(),
        (P::Open, _) => illegal("item is open"),
        (P::InProgress, E::Start) => same(),
        (P::InProgress, E::Produce) => to(P::Produced),
        (P::InProgress, E::Escalate(r)) if *r > state.rung => to(P::Escalated(*r)),
        (P::InProgress, E::Escalate(_)) => illegal("escalation must go to a higher rung"),
        (P::InProgress, E::Requeue) => to(P::Open),
        (P::InProgress, E::ProposeTerminal) => to(P::TerminalCandidate),
        (P::InProgress, _) => illegal("item in progress"),
        (P::Produced, E::Produce) => same(),
        (P::Produced, E::Accept(l)) => to(P::Accepted(*l)),
        (P::Produced, E::Reject(r)) => to(P::Rejected(r.clone())),
        (P::Produced, _) => illegal("produced output awaits acceptance"),
        (P::Accepted(cur), E::Accept(l)) if l == cur => same(),
        (P::Accepted(cur), E::Upgrade(l)) if l > cur => to(P::Accepted(*l)),
        (P::Accepted(cur), E::Upgrade(l)) if l == cur => same(),
        (P::Accepted(_), E::Upgrade(_)) => illegal("evidence level never decreases"),
        (P::Accepted(_), E::Reject(r)) => to(P::Rejected(r.clone())),
        (P::Accepted(_), _) => illegal("item accepted"),
        (P::Rejected(cur), E::Reject(r)) if r == cur => same(),
        (P::Rejected(_), E::Escalate(r)) if *r > state.rung => to(P::Escalated(*r)),
        (P::Rejected(_), E::Escalate(_)) => illegal("escalation must go to a higher rung"),
        (P::Rejected(_), E::Requeue) => to(P::Open),
        (P::Rejected(_), E::ProposeTerminal) => to(P::TerminalCandidate),
        (P::Rejected(_), _) => illegal("item rejected"),
        (P::Escalated(cur), E::Escalate(r)) if r == cur => same(),
        (P::Escalated(_), _) => illegal("item escalated (final; its next-rung item carries the work)"),
        (P::TerminalCandidate, E::ProposeTerminal) => same(),
        (P::TerminalCandidate, E::DeclareTerminal) => to(P::Terminal),
        (P::TerminalCandidate, E::Reopen) => to(P::Open),
        (P::TerminalCandidate, _) => illegal("terminal candidate"),
        (P::Terminal, E::DeclareTerminal) => same(),
        (P::Terminal, _) => illegal("terminal declared (final)"),
        (P::Stale, E::Reopen) => to(P::Open),
        (P::Stale, _) => illegal("item stale (reopen first)"),
    }
}

// ------------------------------------------------------------------------------------------------------------
// (iii) NODE
// ------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodePhase {
    Registered,
    Running,
    ClosureCheck,
    /// Closure fixed point reached (defect 0, no open item) with every accepted rule at least geometry-accepted.
    BoxedClosed,
    /// Boxed-closed AND every accepted rule identity-proved (ruling D4: every used key proved).
    BoxedCertified,
    Stale,
}

impl NodePhase {
    pub fn tag(self) -> &'static str {
        match self {
            NodePhase::Registered => "registered",
            NodePhase::Running => "running",
            NodePhase::ClosureCheck => "closure_check",
            NodePhase::BoxedClosed => "boxed_closed",
            NodePhase::BoxedCertified => "boxed_certified",
            NodePhase::Stale => "stale",
        }
    }
    pub const ALL: [NodePhase; 6] = [
        NodePhase::Registered,
        NodePhase::Running,
        NodePhase::ClosureCheck,
        NodePhase::BoxedClosed,
        NodePhase::BoxedCertified,
        NodePhase::Stale,
    ];
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeState {
    pub phase: NodePhase,
    pub closure_rounds: u32,
    pub last_defect: Option<u64>,
    pub terminals: u32,
    pub terminal_bound: u32,
}

/// Context of a node transition supplied by the reducer (item counters of the node).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NodeContext {
    pub items_open: u64,
    pub items_unproved: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum NodeEvent {
    Start,
    ClosureResult { round: u32, defect_keys: u64, keyset: Option<FileRef> },
    DeclareBoxedClosed,
    DeclareBoxedCertified,
    Reopen { reason: String },
    MarkStale { reason: String },
    /// Internal (emitted by the reducer when an item of the node is declared terminal; never written).
    #[serde(skip)]
    TerminalDeclared { count: u32 },
}

impl NodeEvent {
    pub fn tag(&self) -> &'static str {
        match self {
            NodeEvent::Start => "start",
            NodeEvent::ClosureResult { .. } => "closure_result",
            NodeEvent::DeclareBoxedClosed => "declare_boxed_closed",
            NodeEvent::DeclareBoxedCertified => "declare_boxed_certified",
            NodeEvent::Reopen { .. } => "reopen",
            NodeEvent::MarkStale { .. } => "mark_stale",
            NodeEvent::TerminalDeclared { .. } => "terminal_declared",
        }
    }
}

pub const NODE_TABLE: &[(&str, &str, &str)] = &[
    ("registered", "start", "running"),
    ("running|closure_check", "closure_result", "closure_check  [guard: round > last round]"),
    ("boxed_closed|boxed_certified", "closure_result", "same phase if defect = 0, else closure_check"),
    ("closure_check", "declare_boxed_closed", "boxed_closed  [guard: last defect = 0 and no open item]"),
    ("boxed_closed", "declare_boxed_certified", "boxed_certified  [guard: every accepted item identity-proved]"),
    ("closure_check|boxed_closed|boxed_certified|stale", "reopen", "running"),
    ("<any>", "mark_stale", "stale"),
    ("<any but stale>", "terminal_declared", "same phase, terminals += n  [guard: terminals <= bound]"),
];

pub fn apply_node(state: &NodeState, ev: &NodeEvent, ctx: NodeContext) -> Result<NodeState, IllegalTransition> {
    use NodePhase as P;
    let illegal = |why: String| Err(IllegalTransition::new("node", state.phase.tag(), ev.tag(), why));
    let mut next = state.clone();
    match (state.phase, ev) {
        (_, NodeEvent::MarkStale { .. }) => next.phase = P::Stale,
        (P::Registered, NodeEvent::Start) => next.phase = P::Running,
        (P::Running, NodeEvent::Start) => {}
        (_, NodeEvent::Start) => return illegal("node already started".into()),
        (P::Registered | P::Stale, NodeEvent::ClosureResult { .. }) => {
            return illegal("closure results need a running node".into());
        }
        (phase, NodeEvent::ClosureResult { round, defect_keys, .. }) => {
            if *round == state.closure_rounds && state.last_defect == Some(*defect_keys) {
                return Ok(next); // exact repeat
            }
            if *round <= state.closure_rounds {
                return illegal(format!("closure round {round} not after round {}", state.closure_rounds));
            }
            next.closure_rounds = *round;
            next.last_defect = Some(*defect_keys);
            next.phase = match phase {
                P::BoxedClosed | P::BoxedCertified if *defect_keys == 0 => phase,
                _ => P::ClosureCheck,
            };
        }
        (P::ClosureCheck, NodeEvent::DeclareBoxedClosed) => {
            if state.last_defect != Some(0) {
                return illegal(format!("closure defect is {:?}, not 0", state.last_defect));
            }
            if ctx.items_open > 0 {
                return illegal(format!("{} items still open", ctx.items_open));
            }
            next.phase = P::BoxedClosed;
        }
        (P::BoxedClosed | P::BoxedCertified, NodeEvent::DeclareBoxedClosed) => {}
        (_, NodeEvent::DeclareBoxedClosed) => return illegal("needs a closure check with defect 0".into()),
        (P::BoxedClosed, NodeEvent::DeclareBoxedCertified) => {
            if ctx.items_unproved > 0 {
                return illegal(format!("{} accepted items are not identity-proved", ctx.items_unproved));
            }
            next.phase = P::BoxedCertified;
        }
        (P::BoxedCertified, NodeEvent::DeclareBoxedCertified) => {}
        (_, NodeEvent::DeclareBoxedCertified) => return illegal("needs boxed_closed first".into()),
        (P::ClosureCheck | P::BoxedClosed | P::BoxedCertified | P::Stale, NodeEvent::Reopen { .. }) => {
            next.phase = P::Running
        }
        (P::Running, NodeEvent::Reopen { .. }) => {}
        (P::Registered, NodeEvent::Reopen { .. }) => return illegal("node not started".into()),
        (P::Stale, NodeEvent::TerminalDeclared { .. }) => return illegal("node is stale".into()),
        (_, NodeEvent::TerminalDeclared { count }) => {
            let total = state.terminals.saturating_add(*count);
            if total > state.terminal_bound {
                return illegal(format!(
                    "terminal bound exceeded: {total} > {} (per-node fallback bound)",
                    state.terminal_bound
                ));
            }
            next.terminals = total;
        }
    }
    Ok(next)
}

// ------------------------------------------------------------------------------------------------------------
// (iv) CAMPAIGN
// ------------------------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignPhase {
    Starting,
    Running,
    /// After the first Ctrl-C / SIGTERM: no new launches, running jobs finish or are stopped.
    Draining,
    Stopped,
    Finished,
}

impl CampaignPhase {
    pub fn tag(self) -> &'static str {
        match self {
            CampaignPhase::Starting => "starting",
            CampaignPhase::Running => "running",
            CampaignPhase::Draining => "draining",
            CampaignPhase::Stopped => "stopped",
            CampaignPhase::Finished => "finished",
        }
    }
    pub const ALL: [CampaignPhase; 5] = [
        CampaignPhase::Starting,
        CampaignPhase::Running,
        CampaignPhase::Draining,
        CampaignPhase::Stopped,
        CampaignPhase::Finished,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignEvent {
    /// A supervisor session begins (also after a crash of the previous one: `recovered` in the record).
    Start,
    Run,
    Drain,
    Stop,
    Finish,
}

impl CampaignEvent {
    pub fn tag(self) -> &'static str {
        match self {
            CampaignEvent::Start => "start",
            CampaignEvent::Run => "run",
            CampaignEvent::Drain => "drain",
            CampaignEvent::Stop => "stop",
            CampaignEvent::Finish => "finish",
        }
    }
}

pub const CAMPAIGN_TABLE: &[(&str, &str, &str)] = &[
    ("<created>|stopped|finished", "start", "starting"),
    ("starting|running|draining", "start", "starting  (previous supervisor died: crash recovery)"),
    ("starting", "run", "running"),
    ("starting|running", "drain", "draining"),
    ("starting|running|draining", "stop", "stopped"),
    ("running", "finish", "finished"),
];

/// `None` = created, no session yet.
pub fn apply_campaign(
    state: Option<CampaignPhase>,
    ev: CampaignEvent,
) -> Result<CampaignPhase, IllegalTransition> {
    use CampaignEvent as E;
    use CampaignPhase as P;
    let from = state.map(|s| s.tag()).unwrap_or("created");
    let illegal = |why: &str| Err(IllegalTransition::new("campaign", from, ev.tag(), why));
    match (state, ev) {
        (_, E::Start) => Ok(P::Starting),
        (Some(P::Starting), E::Run) => Ok(P::Running),
        (Some(P::Running), E::Run) => Ok(P::Running),
        (_, E::Run) => illegal("run needs a starting session"),
        (Some(P::Starting | P::Running), E::Drain) => Ok(P::Draining),
        (Some(P::Draining), E::Drain) => Ok(P::Draining),
        (_, E::Drain) => illegal("nothing to drain"),
        (Some(P::Starting | P::Running | P::Draining), E::Stop) => Ok(P::Stopped),
        (Some(P::Stopped), E::Stop) => Ok(P::Stopped),
        (_, E::Stop) => illegal("no session"),
        (Some(P::Running), E::Finish) => Ok(P::Finished),
        (Some(P::Finished), E::Finish) => Ok(P::Finished),
        (_, E::Finish) => illegal("only a running campaign finishes"),
    }
}

// ------------------------------------------------------------------------------------------------------------
// Reduced campaign state
// ------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InputRec {
    pub role: String,
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
    /// Number of distinct digests seen for this input (1 = never changed).
    pub versions: u32,
    pub registered_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRec {
    pub producer: String,
    pub description: String,
    pub files: Vec<FileRef>,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRec {
    pub state: NodeState,
    pub inputs: Vec<String>,
    /// Items of the node per phase, indexed like [`ItemPhase::TAGS`].
    pub items_by_phase: [u64; 9],
    /// Accepted items per evidence level (indexed like [`EvidenceLevel::ALL`]).
    pub accepted_by_level: [u64; 3],
    pub last_keyset: Option<FileRef>,
    pub stale_reason: Option<String>,
    pub cpu_ms: u64,
}

impl NodeRec {
    pub fn context(&self) -> NodeContext {
        let open: u64 = ItemPhase::TAGS
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(**t, "accepted" | "escalated" | "terminal"))
            .map(|(i, _)| self.items_by_phase[i])
            .sum();
        NodeContext { items_open: open, items_unproved: self.accepted_by_level[0] + self.accepted_by_level[1] }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemRec {
    pub node: String,
    pub kind: String,
    pub rung: Rung,
    pub parent: Option<String>,
    pub inputs: Vec<String>,
    pub label: String,
    pub weight: u64,
    pub phase: ItemPhase,
    pub jobs: Vec<String>,
    pub accepted: Option<AcceptedRec>,
    pub stale_from: Option<String>,
    pub stale_input: Option<String>,
    pub terminal_keys: u32,
    pub cpu_ms: u64,
    pub created_seq: u64,
    pub updated_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedRec {
    pub rule: String,
    pub level: EvidenceLevel,
    pub record: FileRef,
    pub source: crate::ledger::AcceptSource,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct JobRec {
    pub item: String,
    pub rung: Rung,
    pub kind: String,
    pub spec_blake3: String,
    pub binary: String,
    pub reserved_bytes: u64,
    pub attempt: u32,
    pub state: AttemptState,
    pub run_dir: Option<String>,
    pub pid: Option<u32>,
    pub start_ticks: Option<u64>,
    pub boot_id: Option<String>,
    pub sealed_table: Option<FileRef>,
    pub time_decided: bool,
    pub cpu_ms: u64,
    pub peak_rss_bytes: u64,
    pub created_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingOp {
    pub action: String,
    pub target: String,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentEvent {
    pub seq: u64,
    pub utc: String,
    pub kind: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCounters {
    pub accepted: u64,
    pub rejected: u64,
    pub rejected_by_reason: BTreeMap<String, u64>,
    pub upgrades: u64,
}

/// Bounded length of the recent-events ring kept in the state (and in the recent-events progress file).
pub const RECENT_CAP: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignState {
    pub campaign_id: String,
    pub spec_blake3: String,
    pub created_utc: String,
    pub terminal_bound: u32,
    pub phase: Option<CampaignPhase>,
    pub last_seq: u64,
    pub last_utc: String,
    pub sessions: u32,
    pub crash_recoveries: u32,
    pub takeovers: u32,
    pub torn_tail_recoveries: u32,
    pub segments_opened: u32,
    pub inputs: BTreeMap<String, InputRec>,
    pub evidence: BTreeMap<String, EvidenceRec>,
    pub nodes: BTreeMap<String, NodeRec>,
    pub items: BTreeMap<String, ItemRec>,
    pub jobs: BTreeMap<String, JobRec>,
    pub pending_ops: BTreeMap<String, PendingOp>,
    pub done_ops: BTreeSet<String>,
    pub rules: RuleCounters,
    pub terminals_total: u64,
    /// CPU milliseconds of every settled attempt per rung (the ETA's samples).
    pub costs_ms: BTreeMap<Rung, Vec<u64>>,
    pub cpu_ms_total: u64,
    pub progress_records: u64,
    pub recent: VecDeque<RecentEvent>,
}

/// Errors of the reducer: an illegal transition, or a record that references something unknown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReduceError {
    Illegal { seq: u64, err: IllegalTransition },
    Invalid { seq: u64, why: String },
}

impl std::fmt::Display for ReduceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReduceError::Illegal { seq, err } => write!(f, "record {seq}: {err}"),
            ReduceError::Invalid { seq, why } => write!(f, "record {seq}: {why}"),
        }
    }
}

impl std::error::Error for ReduceError {}

/// Reduces records (in order) to the campaign state. The ONLY way campaign state is computed.
pub fn reduce<'a>(records: impl IntoIterator<Item = &'a Record>) -> Result<CampaignState, ReduceError> {
    let mut st = CampaignState::default();
    for r in records {
        st.apply_record(r)?;
    }
    Ok(st)
}

impl CampaignState {
    /// Applies one record. All checks happen before any mutation: on `Err` the state is unchanged.
    pub fn apply_record(&mut self, rec: &Record) -> Result<(), ReduceError> {
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let illegal = |err: IllegalTransition| ReduceError::Illegal { seq, err };
        if self.last_seq != 0 && seq != self.last_seq + 1 {
            return Err(invalid(format!("sequence gap: expected {}", self.last_seq + 1)));
        }
        if self.last_seq == 0 && !matches!(rec.event, Event::CampaignCreated { .. }) {
            return Err(invalid("the first record must be campaign_created".into()));
        }
        match &rec.event {
            Event::CampaignCreated { campaign_id, spec_blake3, terminal_bound } => {
                if self.last_seq != 0 {
                    return Err(invalid("campaign_created after the first record".into()));
                }
                self.campaign_id = campaign_id.clone();
                self.spec_blake3 = spec_blake3.clone();
                self.created_utc = rec.utc.clone();
                self.terminal_bound = *terminal_bound;
            }
            Event::Campaign { ev, reason } => {
                let crash = *ev == CampaignEvent::Start
                    && matches!(self.phase, Some(CampaignPhase::Starting | CampaignPhase::Running | CampaignPhase::Draining));
                let next = apply_campaign(self.phase, *ev).map_err(illegal)?;
                if *ev == CampaignEvent::Start {
                    self.sessions += 1;
                    if crash {
                        self.crash_recoveries += 1;
                    }
                }
                if Some(next) != self.phase {
                    self.push_recent(rec, "campaign", format!("campaign {} ({reason})", next.tag()));
                }
                self.phase = Some(next);
            }
            Event::SupervisorStarted { .. } => {}
            Event::LockTakeover { previous, reason, orphans } => {
                self.takeovers += 1;
                self.push_recent(
                    rec,
                    "lock_takeover",
                    format!(
                        "took over lock of dead supervisor pid {} ({reason:?}); {} orphans",
                        previous.identity.pid,
                        orphans.len()
                    ),
                );
            }
            Event::Recovery { truncated_bytes, last_good_seq, .. } => {
                self.torn_tail_recoveries += 1;
                self.push_recent(
                    rec,
                    "recovery",
                    format!("torn ledger tail: {truncated_bytes} bytes dropped after record {last_good_seq}"),
                );
            }
            Event::SegmentOpened { .. } => self.segments_opened += 1,
            Event::InputRegistered { input, role, path, bytes, blake3 } => self.apply_input(rec, input, role, path, *bytes, blake3),
            Event::EvidenceImported { evidence, producer, description, files } => {
                if let Some(e) = self.evidence.get(evidence) {
                    if e.files != *files || e.producer != *producer {
                        return Err(invalid(format!("evidence {evidence} re-imported with different content")));
                    }
                } else {
                    self.evidence.insert(
                        evidence.clone(),
                        EvidenceRec { producer: producer.clone(), description: description.clone(), files: files.clone(), seq },
                    );
                    self.push_recent(rec, "evidence", format!("imported evidence {evidence} by {producer} ({} files)", files.len()));
                }
            }
            Event::NodeRegistered { node, inputs, terminal_bound } => {
                if let Some(n) = self.nodes.get(node) {
                    if n.inputs != *inputs {
                        return Err(invalid(format!("node {node} re-registered with other inputs")));
                    }
                } else {
                    for i in inputs {
                        if !self.inputs.contains_key(i) {
                            return Err(invalid(format!("node {node}: unknown input {i}")));
                        }
                    }
                    self.nodes.insert(
                        node.clone(),
                        NodeRec {
                            state: NodeState {
                                phase: NodePhase::Registered,
                                closure_rounds: 0,
                                last_defect: None,
                                terminals: 0,
                                terminal_bound: terminal_bound.unwrap_or(self.terminal_bound),
                            },
                            inputs: inputs.clone(),
                            items_by_phase: [0; 9],
                            accepted_by_level: [0; 3],
                            last_keyset: None,
                            stale_reason: None,
                            cpu_ms: 0,
                        },
                    );
                }
            }
            Event::Node { node, ev } => {
                if matches!(ev, NodeEvent::TerminalDeclared { .. }) {
                    return Err(invalid("terminal_declared is internal".into()));
                }
                let n = self.nodes.get(node).ok_or_else(|| invalid(format!("unknown node {node}")))?;
                let next = apply_node(&n.state, ev, n.context()).map_err(illegal)?;
                let changed = next != n.state;
                let n = self.nodes.get_mut(node).unwrap();
                n.state = next;
                match ev {
                    NodeEvent::ClosureResult { keyset, .. } if changed => n.last_keyset = keyset.clone(),
                    NodeEvent::MarkStale { reason } => n.stale_reason = Some(reason.clone()),
                    NodeEvent::Reopen { .. } => n.stale_reason = None,
                    _ => {}
                }
                if changed {
                    let text = match ev {
                        NodeEvent::ClosureResult { round, defect_keys, .. } => {
                            format!("{node}: closure round {round}, defect {defect_keys} keys")
                        }
                        _ => format!("{node}: {}", n.state.phase.tag()),
                    };
                    self.push_recent(rec, "node", text);
                }
            }
            Event::ItemCreated { item, node, kind, rung, parent, inputs, label, weight } => {
                if let Some(it) = self.items.get(item) {
                    if it.node != *node || it.rung != *rung || it.kind != *kind {
                        return Err(invalid(format!("item {item} re-created with other identity")));
                    }
                } else {
                    let n = self.nodes.get(node).ok_or_else(|| invalid(format!("item {item}: unknown node {node}")))?;
                    if n.state.phase == NodePhase::Stale {
                        return Err(invalid(format!("item {item}: node {node} is stale")));
                    }
                    if let Some(p) = parent {
                        if !self.items.contains_key(p) {
                            return Err(invalid(format!("item {item}: unknown parent {p}")));
                        }
                    }
                    for i in inputs {
                        if !self.inputs.contains_key(i) {
                            return Err(invalid(format!("item {item}: unknown input {i}")));
                        }
                    }
                    self.nodes.get_mut(node).unwrap().items_by_phase[ItemPhase::Open.index()] += 1;
                    self.items.insert(
                        item.clone(),
                        ItemRec {
                            node: node.clone(),
                            kind: kind.clone(),
                            rung: *rung,
                            parent: parent.clone(),
                            inputs: inputs.clone(),
                            label: label.clone(),
                            weight: *weight,
                            phase: ItemPhase::Open,
                            jobs: Vec::new(),
                            accepted: None,
                            stale_from: None,
                            stale_input: None,
                            terminal_keys: 0,
                            cpu_ms: 0,
                            created_seq: seq,
                            updated_seq: seq,
                        },
                    );
                }
            }
            Event::Item { item, ev } => self.apply_item_event(rec, item, ev)?,
            Event::JobQueued { job, item, rung, kind, spec_blake3, binary, reserved_bytes } => {
                if let Some(j) = self.jobs.get(job) {
                    if j.item != *item || j.spec_blake3 != *spec_blake3 {
                        return Err(invalid(format!("job {job} re-queued with other identity")));
                    }
                } else {
                    let it = self.items.get(item).ok_or_else(|| invalid(format!("job {job}: unknown item {item}")))?;
                    if !it.phase.is_open() || it.phase == ItemPhase::Stale {
                        return Err(invalid(format!("job {job}: item {item} is {}", it.phase.tag())));
                    }
                    if !self.inputs.contains_key(binary) {
                        return Err(invalid(format!("job {job}: binary {binary} not registered (M1)")));
                    }
                    self.items.get_mut(item).unwrap().jobs.push(job.clone());
                    self.jobs.insert(
                        job.clone(),
                        JobRec {
                            item: item.clone(),
                            rung: *rung,
                            kind: kind.clone(),
                            spec_blake3: spec_blake3.clone(),
                            binary: binary.clone(),
                            reserved_bytes: *reserved_bytes,
                            attempt: 1,
                            state: AttemptState::Queued,
                            run_dir: None,
                            pid: None,
                            start_ticks: None,
                            boot_id: None,
                            sealed_table: None,
                            time_decided: false,
                            cpu_ms: 0,
                            peak_rss_bytes: 0,
                            created_seq: seq,
                        },
                    );
                }
            }
            Event::Job { job, attempt, ev } => self.apply_job_event(rec, job, *attempt, ev)?,
            Event::Intent { op, action, target } => {
                if !self.done_ops.contains(op) && !self.pending_ops.contains_key(op) {
                    self.pending_ops.insert(op.clone(), PendingOp { action: action.clone(), target: target.clone(), seq });
                }
            }
            Event::Done { op, .. } => {
                if !self.done_ops.contains(op) {
                    if self.pending_ops.remove(op).is_none() {
                        return Err(invalid(format!("done for unknown operation {op}")));
                    }
                    self.done_ops.insert(op.clone());
                }
            }
            Event::JobProgress { .. } | Event::Note { .. } => self.progress_records += 1,
        }
        self.last_seq = seq;
        self.last_utc = rec.utc.clone();
        Ok(())
    }

    fn apply_input(&mut self, rec: &Record, input: &str, role: &str, path: &str, bytes: u64, blake3: &str) {
        let seq = rec.seq;
        match self.inputs.get_mut(input) {
            Some(cur) if cur.blake3 == blake3 => {} // repeat (or re-verification at resume): no-op
            Some(cur) => {
                let old = std::mem::replace(&mut cur.blake3, blake3.to_string());
                cur.bytes = bytes;
                cur.path = path.to_string();
                cur.versions += 1;
                cur.registered_seq = seq;
                let n = self.mark_stale_by_input(input, seq);
                self.push_recent(
                    rec,
                    "stale_inputs",
                    format!("input {input} changed ({}.. -> {}..): {n} items marked stale", &old[..old.len().min(8)], &blake3[..blake3.len().min(8)]),
                );
            }
            None => {
                self.inputs.insert(
                    input.to_string(),
                    InputRec { role: role.to_string(), path: path.to_string(), bytes, blake3: blake3.to_string(), versions: 1, registered_seq: seq },
                );
            }
        }
    }

    /// Marks stale (never deletes) every item depending on `input`: its own inputs, its node's inputs, or the
    /// binary of the job its acceptance came from. Nodes listing the input are marked stale too.
    fn mark_stale_by_input(&mut self, input: &str, seq: u64) -> usize {
        let nodes_hit: BTreeSet<String> =
            self.nodes.iter().filter(|(_, n)| n.inputs.iter().any(|i| i == input)).map(|(k, _)| k.clone()).collect();
        let mut hit = Vec::new();
        for (id, it) in &self.items {
            if it.phase == ItemPhase::Stale {
                continue;
            }
            let via_job = it.accepted.as_ref().is_some_and(|a| match &a.source {
                crate::ledger::AcceptSource::Job { job, .. } => self.jobs.get(job).is_some_and(|j| j.binary == input),
                _ => false,
            });
            if nodes_hit.contains(&it.node) || it.inputs.iter().any(|i| i == input) || via_job {
                hit.push(id.clone());
            }
        }
        for id in &hit {
            let it = self.items.get_mut(id).unwrap();
            let from = it.phase.clone();
            let n = self.nodes.get_mut(&it.node).unwrap();
            n.items_by_phase[from.index()] -= 1;
            n.items_by_phase[ItemPhase::Stale.index()] += 1;
            if let ItemPhase::Accepted(l) = from {
                n.accepted_by_level[l as usize] -= 1;
            }
            if from == ItemPhase::Terminal {
                n.state.terminals = n.state.terminals.saturating_sub(it.terminal_keys);
                self.terminals_total = self.terminals_total.saturating_sub(it.terminal_keys as u64);
                it.terminal_keys = 0;
            }
            it.stale_from = Some(from.tag().to_string());
            it.stale_input = Some(input.to_string());
            it.phase = ItemPhase::Stale;
            it.updated_seq = seq;
        }
        for node in nodes_hit {
            let n = self.nodes.get_mut(&node).unwrap();
            n.state.phase = NodePhase::Stale;
            n.stale_reason = Some(format!("input {input} changed"));
        }
        hit.len()
    }

    fn apply_item_event(&mut self, rec: &Record, item: &str, ev: &crate::ledger::ItemEvent) -> Result<(), ReduceError> {
        use crate::ledger::{AcceptSource, ItemEvent as IE};
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let illegal = |err: IllegalTransition| ReduceError::Illegal { seq, err };
        let it = self.items.get(item).ok_or_else(|| invalid(format!("unknown item {item}")))?;
        let machine_ev = match ev {
            IE::Start { .. } => ItemMachineEvent::Start,
            IE::Produce { .. } => ItemMachineEvent::Produce,
            IE::Accept { level, .. } => ItemMachineEvent::Accept(*level),
            IE::Upgrade { level, .. } => ItemMachineEvent::Upgrade(*level),
            IE::Reject { reason } => ItemMachineEvent::Reject(reason.clone()),
            IE::Escalate { to, .. } => ItemMachineEvent::Escalate(*to),
            IE::Requeue { .. } => ItemMachineEvent::Requeue,
            IE::ProposeTerminal { .. } => ItemMachineEvent::ProposeTerminal,
            IE::DeclareTerminal { .. } => ItemMachineEvent::DeclareTerminal,
            IE::MarkStale { .. } => ItemMachineEvent::MarkStale,
            IE::Reopen { .. } => ItemMachineEvent::Reopen,
        };
        let cur = ItemState { phase: it.phase.clone(), rung: it.rung };
        let next = apply_item(&cur, &machine_ev).map_err(illegal)?;
        let repeat = next == cur;
        // Additional guards (all before mutation).
        let mut accepted = None;
        match ev {
            IE::Produce { job, attempt, .. } if !repeat => {
                let j = self.jobs.get(job).ok_or_else(|| invalid(format!("produce: unknown job {job}")))?;
                if j.item != item || j.attempt != *attempt || !matches!(j.state, AttemptState::SucceededSealed(_)) {
                    return Err(invalid(format!("produce: job {job} attempt {attempt} is not a sealed success of this item")));
                }
            }
            IE::Accept { rule, level, record, source, elided } => {
                if *elided {
                    return Err(invalid(format!("accept of {rule}: records with elided witness loci are unusable (fail closed)")));
                }
                match source {
                    AcceptSource::Job { job, attempt, table_blake3 } => {
                        let j = self.jobs.get(job).ok_or_else(|| invalid(format!("accept: unknown job {job}")))?;
                        let sealed_ok = matches!(j.state, AttemptState::SucceededSealed(_))
                            && j.attempt == *attempt
                            && j.item == item
                            && j.sealed_table.as_ref().is_some_and(|t| t.blake3 == *table_blake3);
                        if !sealed_ok {
                            return Err(invalid(format!(
                                "accept of {rule}: accepted records come only from a sealed table (job {job} attempt {attempt})"
                            )));
                        }
                    }
                    AcceptSource::Evidence { evidence } => {
                        if !self.evidence.contains_key(evidence) {
                            return Err(invalid(format!("accept of {rule}: unknown evidence {evidence}")));
                        }
                    }
                }
                if repeat {
                    let a = it.accepted.as_ref().unwrap();
                    if a.rule != *rule || a.record != *record {
                        return Err(invalid(format!("item {item} already accepted with rule {}", a.rule)));
                    }
                } else {
                    accepted = Some(AcceptedRec { rule: rule.clone(), level: *level, record: record.clone(), source: source.clone(), seq });
                }
            }
            IE::DeclareTerminal { count, .. } if !repeat => {
                let n = &self.nodes[&it.node];
                apply_node(&n.state, &NodeEvent::TerminalDeclared { count: *count }, n.context()).map_err(illegal)?;
            }
            IE::Escalate { .. } | IE::Reopen { .. } if !repeat => {
                if self.nodes[&it.node].state.phase == NodePhase::Stale && matches!(ev, IE::Reopen { .. }) {
                    return Err(invalid(format!("reopen of {item}: node {} is stale (reopen the node first)", it.node)));
                }
            }
            _ => {}
        }
        if repeat {
            return Ok(());
        }
        // Mutation.
        let it = self.items.get_mut(item).unwrap();
        let from = std::mem::replace(&mut it.phase, next.phase.clone());
        it.updated_seq = seq;
        let node = it.node.clone();
        let n = self.nodes.get_mut(&node).unwrap();
        n.items_by_phase[from.index()] -= 1;
        n.items_by_phase[next.phase.index()] += 1;
        if let ItemPhase::Accepted(l) = from {
            n.accepted_by_level[l as usize] -= 1;
        }
        if let ItemPhase::Accepted(l) = next.phase {
            n.accepted_by_level[l as usize] += 1;
        }
        if n.state.phase == NodePhase::Registered && matches!(ev, IE::Start { .. }) {
            n.state.phase = NodePhase::Running;
        }
        let label = it.label.clone();
        let text = match ev {
            IE::Accept { rule, level, .. } => {
                self.rules.accepted += 1;
                let it = self.items.get_mut(item).unwrap();
                it.accepted = accepted;
                if let AcceptSource::Job { job, .. } = &it.accepted.as_ref().unwrap().source {
                    let b = self.jobs[job].binary.clone();
                    if !it.inputs.contains(&b) {
                        it.inputs.push(b);
                    }
                }
                Some(format!("{node} {label}: rule {rule} accepted ({})", level.word()))
            }
            IE::Upgrade { level, .. } => {
                self.rules.upgrades += 1;
                let it = self.items.get_mut(item).unwrap();
                if let Some(a) = it.accepted.as_mut() {
                    a.level = *level;
                }
                Some(format!("{node} {label}: evidence -> {}", level.word()))
            }
            IE::Reject { reason } => {
                self.rules.rejected += 1;
                *self.rules.rejected_by_reason.entry(reason.clone()).or_default() += 1;
                if matches!(from, ItemPhase::Accepted(_)) {
                    self.items.get_mut(item).unwrap().accepted = None;
                }
                Some(format!("{node} {label}: rejected ({reason})"))
            }
            IE::Escalate { to, reason } => Some(format!("{node} {label}: escalated to {to} ({reason})")),
            IE::DeclareTerminal { count, .. } => {
                let n = self.nodes.get_mut(&node).unwrap();
                n.state.terminals += *count;
                self.terminals_total += *count as u64;
                self.items.get_mut(item).unwrap().terminal_keys = *count;
                Some(format!("{node} {label}: {count} terminal key(s) declared"))
            }
            IE::ProposeTerminal { reason } => Some(format!("{node} {label}: terminal candidate ({reason})")),
            IE::MarkStale { input } => {
                let it = self.items.get_mut(item).unwrap();
                it.stale_from = Some(from.tag().to_string());
                it.stale_input = Some(input.clone());
                if let Some(a) = &it.accepted {
                    let _ = a;
                }
                if from == ItemPhase::Terminal {
                    let k = it.terminal_keys;
                    it.terminal_keys = 0;
                    let n = self.nodes.get_mut(&node).unwrap();
                    n.state.terminals = n.state.terminals.saturating_sub(k);
                    self.terminals_total = self.terminals_total.saturating_sub(k as u64);
                }
                Some(format!("{node} {label}: stale ({input})"))
            }
            IE::Reopen { .. } => {
                let it = self.items.get_mut(item).unwrap();
                it.stale_from = None;
                it.stale_input = None;
                it.accepted = None;
                None
            }
            _ => None,
        };
        if let Some(t) = text {
            let kind = match ev {
                IE::Accept { .. } => "item_accepted",
                IE::Upgrade { .. } => "item_upgraded",
                IE::Reject { .. } => "item_rejected",
                IE::Escalate { .. } => "item_escalated",
                IE::DeclareTerminal { .. } => "terminal_declared",
                IE::ProposeTerminal { .. } => "terminal_proposed",
                _ => "item_stale",
            };
            self.push_recent(rec, kind, t);
        }
        Ok(())
    }

    fn apply_job_event(&mut self, rec: &Record, job: &str, attempt: u32, ev: &crate::ledger::JobEvent) -> Result<(), ReduceError> {
        use crate::ledger::JobEvent as JE;
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let illegal = |err: IllegalTransition| ReduceError::Illegal { seq, err };
        let j = self.jobs.get(job).ok_or_else(|| invalid(format!("unknown job {job}")))?;
        let machine_ev = match ev {
            JE::Launch { .. } => AttemptEvent::Launch,
            JE::Started { .. } => AttemptEvent::Started,
            JE::Finished { class, sealed_table, .. } => AttemptEvent::Finished { class: *class, sealed: sealed_table.is_some() },
            JE::Interrupt { .. } => AttemptEvent::Interrupt,
            JE::Lose { .. } => AttemptEvent::Lose,
            JE::Requeue { .. } => AttemptEvent::Requeue,
            JE::Retry { .. } => AttemptEvent::Retry,
            JE::Cancel { .. } => AttemptEvent::Cancel,
        };
        let new_attempt = matches!(machine_ev, AttemptEvent::Requeue | AttemptEvent::Retry);
        // Replay rule: records of older attempts are ignored; a requeue/retry carries the NEW attempt number.
        if attempt < j.attempt {
            return Ok(());
        }
        if new_attempt {
            if attempt == j.attempt && j.state == AttemptState::Queued {
                return Ok(()); // repeat of the record that created this attempt
            }
            if attempt != j.attempt + 1 {
                return Err(invalid(format!("job {job}: new attempt must be {}, got {attempt}", j.attempt + 1)));
            }
        } else if attempt != j.attempt {
            return Err(invalid(format!("job {job}: event for attempt {attempt}, current is {}", j.attempt)));
        }
        let next = apply_attempt(j.state, machine_ev).map_err(illegal)?;
        if next == j.state && !new_attempt {
            return Ok(()); // exact repeat
        }
        let item = j.item.clone();
        let rung = j.rung;
        let j = self.jobs.get_mut(job).unwrap();
        j.state = next;
        if new_attempt {
            j.attempt = attempt;
            j.run_dir = None;
            j.pid = None;
            j.start_ticks = None;
            j.boot_id = None;
            j.sealed_table = None;
            j.time_decided = false;
        }
        match ev {
            JE::Launch { run_dir, .. } => j.run_dir = Some(run_dir.clone()),
            JE::Started { pid, start_ticks, boot_id } => {
                j.pid = Some(*pid);
                j.start_ticks = Some(*start_ticks);
                j.boot_id = Some(boot_id.clone());
            }
            JE::Finished { class, sealed_table, cpu_ms, peak_rss_bytes, time_decided, .. } => {
                j.sealed_table = sealed_table.clone();
                j.time_decided = *time_decided;
                j.cpu_ms += *cpu_ms;
                j.peak_rss_bytes = j.peak_rss_bytes.max(*peak_rss_bytes);
                self.cpu_ms_total += *cpu_ms;
                if !matches!(class, FinishClass::Interrupted | FinishClass::Lost) {
                    self.costs_ms.entry(rung).or_default().push(*cpu_ms);
                }
                if let Some(it) = self.items.get_mut(&item) {
                    it.cpu_ms += *cpu_ms;
                    let node = it.node.clone();
                    let label = it.label.clone();
                    if let Some(n) = self.nodes.get_mut(&node) {
                        n.cpu_ms += *cpu_ms;
                    }
                    self.push_recent(rec, "job_finished", format!("{node} {label} {rung}: job {} ({})", short(job), class.word()));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn push_recent(&mut self, rec: &Record, kind: &str, text: String) {
        if self.recent.len() == RECENT_CAP {
            self.recent.pop_front();
        }
        self.recent.push_back(RecentEvent { seq: rec.seq, utc: rec.utc.clone(), kind: kind.to_string(), text });
    }

    /// Attempts that may still have a process (launching / running): what a resuming supervisor must settle
    /// (orphan check by identity, then `interrupt` / `lose` records, then `requeue`).
    pub fn unsettled_jobs(&self) -> Vec<(&String, &JobRec)> {
        self.jobs.iter().filter(|(_, j)| !j.state.is_settled()).collect()
    }

    /// Items with phase counts over the whole campaign, indexed like [`ItemPhase::TAGS`].
    pub fn items_by_phase(&self) -> [u64; 9] {
        let mut out = [0; 9];
        for n in self.nodes.values() {
            for (o, v) in out.iter_mut().zip(n.items_by_phase) {
                *o += v;
            }
        }
        out
    }

    /// Stable digest of the state (blake3 of its JSON): used to compare snapshot + tail with full replay.
    pub fn digest(&self) -> String {
        blake3::hash(&serde_json::to_vec(self).expect("state serialises")).to_hex().to_string()
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// splitmix64: tiny deterministic generator for property tests (no external crate).
    pub(crate) struct Rng(u64);
    impl Rng {
        pub fn new(seed: u64) -> Self {
            Rng(seed)
        }
        pub fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        pub fn below(&mut self, n: usize) -> usize {
            (self.next() % n as u64) as usize
        }
    }

    fn all_attempt_states() -> Vec<AttemptState> {
        let mut v = vec![
            AttemptState::Queued,
            AttemptState::Launching,
            AttemptState::Running,
            AttemptState::Interrupted,
            AttemptState::Lost,
            AttemptState::Cancelled,
        ];
        for c in FinishClass::ALL {
            match c.target() {
                AttemptTarget::SucceededSealed => v.push(AttemptState::SucceededSealed(c)),
                AttemptTarget::BudgetStopped => v.push(AttemptState::BudgetStopped(c)),
                AttemptTarget::Failed => v.push(AttemptState::Failed(c)),
                _ => {}
            }
        }
        v
    }

    fn all_attempt_events() -> Vec<AttemptEvent> {
        let mut v = vec![
            AttemptEvent::Launch,
            AttemptEvent::Started,
            AttemptEvent::Interrupt,
            AttemptEvent::Lose,
            AttemptEvent::Requeue,
            AttemptEvent::Retry,
            AttemptEvent::Cancel,
        ];
        for c in FinishClass::ALL {
            v.push(AttemptEvent::Finished { class: c, sealed: true });
            v.push(AttemptEvent::Finished { class: c, sealed: false });
        }
        v
    }

    fn table_has(table: &[(&str, &str, &str)], from: &str, ev: &str) -> bool {
        table.iter().any(|(f, e, _)| *e == ev && (f.split('|').any(|x| x == from) || f.starts_with("<any")))
    }

    #[test]
    fn attempt_machine_matches_its_table_exhaustively() {
        for s in all_attempt_states() {
            for e in all_attempt_events() {
                match apply_attempt(s, e) {
                    Ok(n) if n == s => {} // repeat: allowed only for the event that produced s (checked below)
                    Ok(_) => assert!(table_has(ATTEMPT_TABLE, s.tag(), e.tag()), "{s:?} --{e:?}--> not in table"),
                    Err(_) => {
                        // a listed (from, event) may only fail on the seal guard
                        if table_has(ATTEMPT_TABLE, s.tag(), e.tag()) {
                            assert!(matches!(e, AttemptEvent::Finished { sealed: false, .. }), "{s:?} {e:?}");
                        }
                    }
                }
            }
        }
        // settled states never go back to a live state
        for s in all_attempt_states().into_iter().filter(|s| s.is_settled() && *s != AttemptState::Queued) {
            for e in all_attempt_events() {
                if let Ok(n) = apply_attempt(s, e) {
                    assert!(n.is_settled(), "{s:?} -> {n:?}");
                }
            }
        }
        // success needs a seal
        assert!(apply_attempt(AttemptState::Running, AttemptEvent::Finished { class: FinishClass::RuleFound, sealed: false }).is_err());
    }

    #[test]
    fn attempt_random_walks_respect_the_table() {
        let states = all_attempt_states();
        let events = all_attempt_events();
        let mut rng = Rng::new(0x5eed);
        for _ in 0..2000 {
            let mut s = states[rng.below(states.len())];
            for _ in 0..30 {
                let e = events[rng.below(events.len())];
                if let Ok(n) = apply_attempt(s, e) {
                    assert!(n == s || table_has(ATTEMPT_TABLE, s.tag(), e.tag()));
                    s = n;
                }
            }
        }
    }

    fn item_events() -> Vec<ItemMachineEvent> {
        use ItemMachineEvent as E;
        let mut v = vec![E::Start, E::Produce, E::Requeue, E::ProposeTerminal, E::DeclareTerminal, E::MarkStale, E::Reopen];
        for l in EvidenceLevel::ALL {
            v.push(E::Accept(l));
            v.push(E::Upgrade(l));
        }
        v.push(E::Reject("cert_failed".into()));
        v.push(E::Reject("rule_covers_none".into()));
        v.push(E::Escalate(Rung(0)));
        v.push(E::Escalate(Rung(1)));
        v.push(E::Escalate(Rung(3)));
        v
    }

    fn item_phases() -> Vec<ItemPhase> {
        let mut v = vec![ItemPhase::Open, ItemPhase::InProgress, ItemPhase::Produced, ItemPhase::TerminalCandidate, ItemPhase::Terminal, ItemPhase::Stale];
        for l in EvidenceLevel::ALL {
            v.push(ItemPhase::Accepted(l));
        }
        v.push(ItemPhase::Rejected("cert_failed".into()));
        v.push(ItemPhase::Escalated(Rung(2)));
        v
    }

    #[test]
    fn item_machine_matches_its_table_exhaustively() {
        for p in item_phases() {
            let s = ItemState { phase: p.clone(), rung: Rung(1) };
            for e in item_events() {
                match apply_item(&s, &e) {
                    Ok(n) if n == s => {}
                    Ok(n) => {
                        assert!(table_has(ITEM_TABLE, s.phase.tag(), e.tag()), "{p:?} --{e:?}--> {n:?} not in table");
                        if let ItemMachineEvent::Escalate(r) = e {
                            assert!(r > s.rung);
                        }
                        if let (ItemPhase::Accepted(a), ItemPhase::Accepted(b)) = (&s.phase, &n.phase) {
                            assert!(b > a, "level decreased");
                        }
                    }
                    Err(_) => {}
                }
            }
        }
        // terminal and escalated are final except for staleness
        for p in [ItemPhase::Terminal, ItemPhase::Escalated(Rung(2))] {
            let s = ItemState { phase: p, rung: Rung(1) };
            for e in item_events() {
                if let Ok(n) = apply_item(&s, &e) {
                    assert!(n == s || n.phase == ItemPhase::Stale);
                }
            }
        }
    }

    #[test]
    fn node_and_campaign_machines() {
        let base = NodeState { phase: NodePhase::Registered, closure_rounds: 0, last_defect: None, terminals: 0, terminal_bound: 2 };
        let ctx = NodeContext::default();
        let s = apply_node(&base, &NodeEvent::Start, ctx).unwrap();
        assert!(apply_node(&s, &NodeEvent::DeclareBoxedClosed, ctx).is_err());
        let s = apply_node(&s, &NodeEvent::ClosureResult { round: 1, defect_keys: 5, keyset: None }, ctx).unwrap();
        assert!(apply_node(&s, &NodeEvent::DeclareBoxedClosed, ctx).is_err());
        assert!(apply_node(&s, &NodeEvent::ClosureResult { round: 1, defect_keys: 0, keyset: None }, ctx).is_err());
        let s = apply_node(&s, &NodeEvent::ClosureResult { round: 2, defect_keys: 0, keyset: None }, ctx).unwrap();
        assert!(apply_node(&s, &NodeEvent::DeclareBoxedClosed, NodeContext { items_open: 1, items_unproved: 0 }).is_err());
        let s = apply_node(&s, &NodeEvent::DeclareBoxedClosed, ctx).unwrap();
        assert_eq!(s.phase, NodePhase::BoxedClosed);
        assert!(apply_node(&s, &NodeEvent::DeclareBoxedCertified, NodeContext { items_open: 0, items_unproved: 3 }).is_err());
        let s2 = apply_node(&s, &NodeEvent::DeclareBoxedCertified, ctx).unwrap();
        assert_eq!(s2.phase, NodePhase::BoxedCertified);
        let t = apply_node(&s, &NodeEvent::TerminalDeclared { count: 2 }, ctx).unwrap();
        assert!(apply_node(&t, &NodeEvent::TerminalDeclared { count: 1 }, ctx).is_err(), "bound 2");
        // campaign
        use CampaignEvent as E;
        let p = apply_campaign(None, E::Start).unwrap();
        assert!(apply_campaign(None, E::Run).is_err());
        let p = apply_campaign(Some(p), E::Run).unwrap();
        let d = apply_campaign(Some(p), E::Drain).unwrap();
        assert!(apply_campaign(Some(d), E::Finish).is_err());
        let st = apply_campaign(Some(d), E::Stop).unwrap();
        assert_eq!(apply_campaign(Some(st), E::Start).unwrap(), CampaignPhase::Starting);
        for s in CampaignPhase::ALL {
            for e in [E::Start, E::Run, E::Drain, E::Stop, E::Finish] {
                if let Ok(n) = apply_campaign(Some(s), e) {
                    assert!(n == s || table_has(CAMPAIGN_TABLE, s.tag(), e.tag()), "{s:?} {e:?}");
                }
            }
        }
        for s in NodePhase::ALL {
            let st = NodeState { phase: s, ..base.clone() };
            for e in [NodeEvent::Start, NodeEvent::DeclareBoxedClosed, NodeEvent::DeclareBoxedCertified, NodeEvent::Reopen { reason: "x".into() }] {
                if let Ok(n) = apply_node(&st, &e, ctx) {
                    assert!(n == st || NODE_TABLE.iter().any(|(f, ev, _)| *ev == e.tag() && f.split('|').any(|x| x == s.tag())), "{s:?} {e:?}");
                }
            }
        }
    }
}
