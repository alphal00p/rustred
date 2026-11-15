//! State machines of the campaign and the reducer that computes the campaign state from the ledger.
//!
//! * Every machine is a pure function `apply(state, event[, context]) -> Result<state, IllegalTransition>` with
//!   an exhaustive `match` and a transition table (`*_TABLE`) that the tests check against `apply`.
//! * [`reduce`] / [`CampaignState::apply_record`] is the ONLY way campaign state is computed: the supervisor
//!   keeps no truth outside the ledger; the in-memory state of a [`crate::ledger::Ledger`] is a cache of the
//!   reduction of its records.
//! * GOVERNING PRINCIPLE: whatever sequence of records a (possibly buggy) supervisor appends, every forbidden
//!   state is unreachable: an illegal record is refused (typed error, nothing written) and all checks precede
//!   any mutation. Verdicts that depend on other facts (a node's closed / certified, a result's validity) are
//!   bound to those facts and withdrawn automatically when they change.
//! * Input identity (audit A C1): every job is stamped with the digests of its inputs and the generation of its
//!   item; a produced result / acceptance is legal only from an attempt of the item's CURRENT generation whose
//!   inputs have not changed since. A changed input marks dependent jobs, results and accepts STALE (never
//!   deleted), transitively through child items.
//! * Acceptance legality (audit A C3) comes from the contract's decision table through ONE exhaustive mapping
//!   ([`contract_accepts`]); a new contract class fails to compile here instead of defaulting.
//!
//! No node names, counts, arities or master numbers appear here (R41 P-5, U-9): everything of that kind is a
//! spec value or input data; comparison numbers live only in the comparison-only `reference` field.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::ledger::{
    AcceptSource, CampaignMode, ControlEvent, ControlTarget, ControlVerb, EnvelopeKind, Event,
    ExportEvent, FileRef, NodeSpecValues, Record, RetirementProof, RuleEconomy,
};

mod cowmap;
pub use cowmap::CowMap;

// ------------------------------------------------------------------------------------------------------------
// Common vocabulary
// ------------------------------------------------------------------------------------------------------------

/// A ladder rung `L0`..`L9`.
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

/// Evidence level of an accepted rule: geometry alone, an independent identity test (certification level
/// `replay`), an exact per-key identity proof (`per-key`). Ordered: a level only ever increases.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceLevel {
    GeometryAccepted,
    IdentityTested,
    IdentityProved,
}

impl EvidenceLevel {
    pub const ALL: [EvidenceLevel; 3] = [
        EvidenceLevel::GeometryAccepted,
        EvidenceLevel::IdentityTested,
        EvidenceLevel::IdentityProved,
    ];
    pub fn word(self) -> &'static str {
        match self {
            EvidenceLevel::GeometryAccepted => "geometry_accepted",
            EvidenceLevel::IdentityTested => "identity_tested",
            EvidenceLevel::IdentityProved => "identity_proved",
        }
    }
}

/// Certification level of a node: a PER-NODE spec value (U-12, R41 P-3). `replay` = identity replay at two primes
/// distinct from the reconstruction primes + descent + guard / pole checks (= `identity_tested` per rule);
/// `per-key` = the exact per-key proof (= `identity_proved` per rule). Defaults: FULL -> replay, BOXED -> per-key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum CertLevel {
    #[serde(rename = "replay")]
    Replay,
    #[serde(rename = "per-key")]
    PerKey,
}

impl CertLevel {
    pub fn required_level(self) -> EvidenceLevel {
        match self {
            CertLevel::Replay => EvidenceLevel::IdentityTested,
            CertLevel::PerKey => EvidenceLevel::IdentityProved,
        }
    }
    pub fn word(self) -> &'static str {
        match self {
            CertLevel::Replay => "replay",
            CertLevel::PerKey => "per-key",
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
    fn new(
        machine: &'static str,
        from: impl Into<String>,
        event: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        IllegalTransition {
            machine,
            from: from.into(),
            event: event.into(),
            why: why.into(),
        }
    }
}

impl std::fmt::Display for IllegalTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "illegal {} transition: {} --{}--> ({})",
            self.machine, self.from, self.event, self.why
        )
    }
}

impl std::error::Error for IllegalTransition {}

// ------------------------------------------------------------------------------------------------------------
// (i) JOB ATTEMPT
// ------------------------------------------------------------------------------------------------------------

/// The contract's "what may be accepted" column of the decision table.
pub use rustred_campaign_contract::classify::Accept as ContractAccept;
/// Outcome kinds of a finished attempt: the contract crate's classification (child-job.md 6.5), used directly.
pub use rustred_campaign_contract::classify::OutcomeKind as FinishClass;

/// Every outcome kind (for exhaustive tests).
pub const FINISH_CLASSES: [FinishClass; 16] = [
    FinishClass::RuleFound,
    FinishClass::Complete,
    FinishClass::NoRule,
    FinishClass::Spill,
    FinishClass::Refused,
    FinishClass::KilledTime,
    FinishClass::KilledMemory,
    FinishClass::Interrupted,
    FinishClass::Crashed,
    FinishClass::UsageError,
    FinishClass::SetupError,
    FinishClass::Lost,
    FinishClass::DecidedNegative,
    FinishClass::Undecided,
    FinishClass::SealedWithElided,
    FinishClass::UnverifiableKind,
];

/// THE mapping from the contract's classification to acceptance legality: the `accept` values the decision
/// table of child-job.md 6.5 allows for each class (rows cited). Exhaustive on purpose: a new contract class
/// (e.g. `sealed_with_elided`, never acceptable) must be added here or the crate does not compile.
/// Spill (row 8, "c=20, O, S": exit 20 with a SEALED partial table, child-job.md 3.4.4 item 2 "records are
/// ACCEPTED only from a sealed table of a job with status complete, incomplete or budget with a WORK counter
/// (exit 20)") may accept its committed rules; nothing is ever salvaged from an interrupted attempt (R-b).
pub fn contract_accepts(class: FinishClass) -> &'static [ContractAccept] {
    use ContractAccept as A;
    use FinishClass::*;
    match class {
        Complete => &[A::Canonical, A::Nothing],    // rows 1, 3
        NoRule => &[A::CommittedRules, A::Nothing], // rows 4, 24
        Spill => &[A::CommittedRules],              // row 8
        DecidedNegative => &[A::Nothing],           // row 7 (c=11)
        Undecided => &[A::Nothing],                 // row 7 (c=12)
        KilledTime => &[A::Nothing],                // rows 9, 19
        KilledMemory => &[A::Nothing],              // rows 10, 18
        Interrupted => &[A::Nothing],               // rows 11, 20
        UsageError => &[A::Nothing],                // row 12
        Refused => &[A::Nothing],                   // rows 13, 14
        Lost => &[A::Nothing],                      // rows 15, 21
        Crashed => &[A::Nothing],                   // rows 6, 16, 17, 22, 23
        SetupError => &[A::Nothing],
        // rows 2, 5; 28 = kind solve-recipe (lane adapters)
        RuleFound => &[A::Canonical, A::SolvedRecords],
        SealedWithElided => &[A::Nothing], // row 25 (elided witness loci: never used, R-f)
        UnverifiableKind => &[A::Nothing], // row 26 (no sealing rule for the kind)
    }
}

/// Whether an `accept` value lets records come from the attempt's sealed table.
pub fn accept_permits_records(a: ContractAccept) -> bool {
    match a {
        ContractAccept::Canonical | ContractAccept::CommittedRules => true,
        ContractAccept::SolvedRecords => true, // row 28 (lane adapters)
        // line 7 (rows 30-32): diagnostic records are never rule or relation evidence; no record is taken from them here
        ContractAccept::DiagnosticRecords => false,
        ContractAccept::Nothing => false,
    }
}

pub fn accept_word(a: ContractAccept) -> &'static str {
    match a {
        ContractAccept::Canonical => "canonical",
        ContractAccept::CommittedRules => "committed_rules",
        ContractAccept::SolvedRecords => "solved_records", // lane adapters (row 28)
        ContractAccept::DiagnosticRecords => "diagnostic_records", // line 7 (rows 30-32)
        ContractAccept::Nothing => "nothing",
    }
}

/// Terminal attempt state reached by an outcome kind (the mapping onto the attempt machine's states).
pub fn class_target(c: FinishClass) -> AttemptTarget {
    use FinishClass::*;
    match c {
        Complete | RuleFound | NoRule | DecidedNegative | Undecided => {
            AttemptTarget::SucceededSealed
        }
        Spill | KilledTime | KilledMemory => AttemptTarget::BudgetStopped,
        Interrupted => AttemptTarget::Interrupted,
        Lost => AttemptTarget::Lost,
        UsageError | SetupError | Refused | Crashed | SealedWithElided | UnverifiableKind => {
            AttemptTarget::Failed
        }
    }
}

/// The serde word of an outcome kind.
pub fn class_word(c: FinishClass) -> &'static str {
    use FinishClass::*;
    match c {
        RuleFound => "rule_found",
        Complete => "complete",
        NoRule => "no_rule",
        Spill => "spill",
        Refused => "refused",
        KilledTime => "killed_time",
        KilledMemory => "killed_memory",
        Interrupted => "interrupted",
        Crashed => "crashed",
        UsageError => "usage_error",
        SetupError => "setup_error",
        Lost => "lost",
        DecidedNegative => "decided_negative",
        Undecided => "undecided",
        SealedWithElided => "sealed_with_elided",
        UnverifiableKind => "unverifiable_kind",
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

/// State of the CURRENT attempt of a job. (`succeeded_sealed` = settled with a success class; whether records
/// may come from it is the separate `accept` of the finish record.)
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
    /// EXECUTION UNKNOWN (an unconfirmed launch): the launch intent was journalled (`job.launch`) but its start was
    /// never confirmed (`job.started`) before the supervisor that wrote it ended. The child may have run and may
    /// have sealed results. Never "did not run" (no interrupt / lose / requeue / retry), never retried
    /// automatically; it leaves only by a late confirmation (`started`), by the authenticated outcome of the run
    /// directory (`finished` with the OUTCOME binding of exactly this launch), or by `cancel` once an operator
    /// decision or an input change has superseded the work. Not settled: a campaign cannot finish over it.
    Unconfirmed,
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
            AttemptState::Unconfirmed => "unconfirmed",
        }
    }
    pub const TAGS: [&'static str; 10] = [
        "queued",
        "launching",
        "running",
        "succeeded_sealed",
        "failed",
        "budget_stopped",
        "interrupted",
        "lost",
        "cancelled",
        "unconfirmed",
    ];
    /// True when no process can exist for this attempt any more and its outcome is known. An unconfirmed launch
    /// is NOT settled (a process or sealed results may exist).
    pub fn is_settled(&self) -> bool {
        !matches!(
            self,
            AttemptState::Launching | AttemptState::Running | AttemptState::Unconfirmed
        )
    }
}

/// Attempt-level events (payload-free view used by the machine; the ledger's [`crate::ledger::JobEvent`]
/// carries the data).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttemptEvent {
    Launch,
    Started,
    Finished {
        class: FinishClass,
        accept: ContractAccept,
        sealed: bool,
    },
    Interrupt,
    Lose,
    /// New attempt after `interrupted` / `lost` (campaign resume; never counted as a failure, U-5).
    Requeue,
    /// New attempt after `failed` (retry policy).
    Retry,
    Cancel,
    /// The launch intent of an earlier session was never confirmed: execution unknown.
    Unconfirmed,
    /// The spawn of an unconfirmed launch provably never happened (authenticated run directory without the spawn
    /// sentinel, no marked process): the only "did not run" (lane int5).
    NotSpawned,
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
            AttemptEvent::Unconfirmed => "unconfirmed",
            AttemptEvent::NotSpawned => "not_spawned",
        }
    }
}

/// Transition table of the attempt machine: (from, event, to). `finished:*` expands to the class target.
pub const ATTEMPT_TABLE: &[(&str, &str, &str)] = &[
    ("queued", "launch", "launching"),
    ("queued", "cancel", "cancelled"),
    ("launching", "started", "running"),
    (
        "launching",
        "finished",
        "<class target>  [guards: accept allowed for the class; records-accepting needs a seal]",
    ),
    ("launching", "interrupt", "interrupted"),
    ("launching", "lose", "lost"),
    (
        "running",
        "finished",
        "<class target>  [guards: accept allowed for the class; records-accepting needs a seal]",
    ),
    ("running", "interrupt", "interrupted"),
    ("running", "lose", "lost"),
    ("interrupted", "requeue", "queued (attempt + 1)"),
    ("lost", "requeue", "queued (attempt + 1)"),
    ("failed", "retry", "queued (attempt + 1)"),
    (
        "launching",
        "unconfirmed",
        "unconfirmed  (execution unknown: the start was never confirmed)",
    ),
    (
        "unconfirmed",
        "started",
        "running  (late confirmation: the recorded child is found and adopted)",
    ),
    (
        "unconfirmed",
        "finished",
        "<class target>  [reducer: the OUTCOME binding of exactly this launch + the OUTCOME file]",
    ),
    (
        "unconfirmed",
        "cancel",
        "cancelled  [reducer: the job can no longer feed its item (operator retry / input change)]",
    ),
    (
        "unconfirmed",
        "not_spawned",
        "lost  (the spawn provably never happened: authenticated run directory without RUN/sup/SPAWN; then requeue)",
    ),
];

/// The attempt machine. `Requeue` / `Retry` return `Queued` (the caller increments the attempt number).
pub fn apply_attempt(
    state: AttemptState,
    ev: AttemptEvent,
) -> Result<AttemptState, IllegalTransition> {
    use AttemptEvent as E;
    use AttemptState as S;
    let illegal = |why: &str| {
        Err(IllegalTransition::new(
            "job attempt",
            state.tag(),
            ev.tag(),
            why,
        ))
    };
    let finish = |class: FinishClass,
                  accept: ContractAccept,
                  sealed: bool|
     -> Result<AttemptState, IllegalTransition> {
        let fail = |why: String| {
            Err(IllegalTransition::new(
                "job attempt",
                state.tag(),
                "finished",
                why,
            ))
        };
        if !contract_accepts(class).contains(&accept) {
            return fail(format!(
                "the contract's row for class {} never says accept {}",
                class_word(class),
                accept_word(accept)
            ));
        }
        if accept_permits_records(accept) && !sealed {
            return fail(format!(
                "accept {} needs a sealed output",
                accept_word(accept)
            ));
        }
        Ok(match class_target(class) {
            AttemptTarget::SucceededSealed => S::SucceededSealed(class),
            AttemptTarget::BudgetStopped => S::BudgetStopped(class),
            AttemptTarget::Interrupted => S::Interrupted,
            AttemptTarget::Lost => S::Lost,
            AttemptTarget::Failed => S::Failed(class),
        })
    };
    match (state, ev) {
        (S::Queued, E::Launch) => Ok(S::Launching),
        (S::Queued, E::Cancel) => Ok(S::Cancelled),
        (S::Queued, E::Requeue | E::Retry) => Ok(S::Queued), // repeat of the record that created this attempt
        (S::Queued, _) => illegal("attempt not launched"),
        (S::Launching, E::Launch) => Ok(S::Launching),
        (S::Launching, E::Unconfirmed) => Ok(S::Unconfirmed),
        (S::Unconfirmed, E::Unconfirmed) => Ok(state),
        (S::Unconfirmed, E::Started) => Ok(S::Running),
        (
            S::Unconfirmed,
            E::Finished {
                class,
                accept,
                sealed,
            },
        ) => finish(class, accept, sealed),
        (S::Unconfirmed, E::Cancel) => Ok(S::Cancelled),
        (S::Unconfirmed, E::NotSpawned) => Ok(S::Lost),
        (S::Unconfirmed, _) => illegal(
            "execution unknown (unconfirmed launch): never 'did not run', never retried automatically; only a late start, the authenticated outcome or a cancel after the work was superseded",
        ),
        (S::Launching, E::Started) => Ok(S::Running),
        (
            S::Launching | S::Running,
            E::Finished {
                class,
                accept,
                sealed,
            },
        ) => finish(class, accept, sealed),
        (S::Launching | S::Running, E::Interrupt) => Ok(S::Interrupted),
        (S::Launching | S::Running, E::Lose) => Ok(S::Lost),
        (S::Launching, _) => illegal("attempt is launching"),
        (S::Running, E::Started) => Ok(S::Running),
        (S::Running, _) => illegal("attempt is running"),
        (S::SucceededSealed(c), E::Finished { class, .. }) if c == class => Ok(state),
        (S::SucceededSealed(_), _) => illegal("attempt succeeded (final)"),
        (S::BudgetStopped(c), E::Finished { class, .. }) if c == class => Ok(state),
        (S::BudgetStopped(_), _) => {
            illegal("attempt stopped by budget (final; the item escalates)")
        }
        (S::Failed(c), E::Finished { class, .. }) if c == class => Ok(state),
        (S::Failed(_), E::Retry) => Ok(S::Queued),
        (S::Failed(_), _) => illegal("attempt failed (only retry)"),
        (
            S::Interrupted,
            E::Interrupt
            | E::Finished {
                class: FinishClass::Interrupted,
                ..
            },
        ) => Ok(state),
        (S::Interrupted, E::Requeue) => Ok(S::Queued),
        (S::Interrupted, _) => illegal("attempt interrupted (only requeue)"),
        (
            S::Lost,
            E::Lose
            | E::Finished {
                class: FinishClass::Lost,
                ..
            },
        ) => Ok(state),
        (S::Lost, E::Requeue) => Ok(S::Queued),
        (S::Lost, _) => illegal("attempt lost (only requeue)"),
        (S::Cancelled, E::Cancel) => Ok(state),
        (S::Cancelled, _) => illegal("job cancelled (final)"),
    }
}

// ------------------------------------------------------------------------------------------------------------
// (ii) WORK ITEM
// ------------------------------------------------------------------------------------------------------------

/// Item phase. An item is one unit of work at ONE rung; escalation ends the item and the supervisor creates the
/// next-rung item with `parent` = this one.
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
    /// Retired by a coverage-preservation proof (or, on a BOXED node only, a box closure pass).
    Retired,
    /// Needs a person (reason code).
    Attention(String),
    /// Skipped by an operator: its work stays OPEN (the node cannot close unless covered otherwise).
    Skipped,
}

/// Number of item phases (length of the per-phase counter arrays).
pub const N_ITEM_PHASES: usize = 12;

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
            ItemPhase::Retired => "retired",
            ItemPhase::Attention(_) => "attention",
            ItemPhase::Skipped => "skipped",
        }
    }
    pub const TAGS: [&'static str; N_ITEM_PHASES] = [
        "open",
        "in_progress",
        "produced",
        "accepted",
        "rejected",
        "escalated",
        "terminal_candidate",
        "terminal",
        "stale",
        "retired",
        "attention",
        "skipped",
    ];
    pub fn index(&self) -> usize {
        Self::TAGS.iter().position(|t| *t == self.tag()).unwrap()
    }
    /// Open work: everything that is not accepted, escalated (superseded by its child), terminal or retired.
    pub fn is_open(&self) -> bool {
        !matches!(
            self,
            ItemPhase::Accepted(_)
                | ItemPhase::Escalated(_)
                | ItemPhase::Terminal
                | ItemPhase::Retired
        )
    }
}

/// Item events (payload-free view; the ledger's [`crate::ledger::ItemEvent`] carries the data).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemMachineEvent {
    Start,
    Produce,
    Accept(EvidenceLevel),
    Import(EvidenceLevel),
    Upgrade(EvidenceLevel),
    Reject(String),
    Escalate(Rung),
    Requeue,
    ProposeTerminal,
    DeclareTerminal,
    MarkStale,
    Reopen,
    Retire,
    Reinstate(EvidenceLevel),
    Attention(String),
    /// Operator control `retry` (applied).
    ControlRetry,
    /// Operator control `skip` (applied).
    ControlSkip,
}

impl ItemMachineEvent {
    pub fn tag(&self) -> &'static str {
        match self {
            ItemMachineEvent::Start => "start",
            ItemMachineEvent::Produce => "produce",
            ItemMachineEvent::Accept(_) => "accept",
            ItemMachineEvent::Import(_) => "import",
            ItemMachineEvent::Upgrade(_) => "upgrade",
            ItemMachineEvent::Reject(_) => "reject",
            ItemMachineEvent::Escalate(_) => "escalate",
            ItemMachineEvent::Requeue => "requeue",
            ItemMachineEvent::ProposeTerminal => "propose_terminal",
            ItemMachineEvent::DeclareTerminal => "declare_terminal",
            ItemMachineEvent::MarkStale => "mark_stale",
            ItemMachineEvent::Reopen => "reopen",
            ItemMachineEvent::Retire => "retire",
            ItemMachineEvent::Reinstate(_) => "reinstate",
            ItemMachineEvent::Attention(_) => "attention",
            ItemMachineEvent::ControlRetry => "control_retry",
            ItemMachineEvent::ControlSkip => "control_skip",
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
    (
        "open",
        "propose_terminal",
        "terminal_candidate  [reducer: preferred, or ladder exhausted at the spec's terminal rung]",
    ),
    (
        "open",
        "import",
        "accepted(level)  [reducer: provenance record, level <= its claim]",
    ),
    (
        "in_progress",
        "produce",
        "produced  [reducer: current-generation, non-stale attempt whose contract row accepts records]",
    ),
    ("in_progress", "escalate", "escalated  [guard: to > rung]"),
    ("in_progress", "requeue", "open  (generation + 1)"),
    ("in_progress", "propose_terminal", "terminal_candidate"),
    (
        "produced",
        "accept",
        "accepted(level)  [reducer: the producing attempt, evidence artefacts for the level]",
    ),
    ("produced", "reject", "rejected(reason)"),
    (
        "accepted",
        "upgrade",
        "accepted(higher level)  [guard: level > current; reducer: evidence artefacts]",
    ),
    ("accepted", "reject", "rejected(reason)"),
    (
        "accepted",
        "retire",
        "retired  [reducer: retirement proof; box-closure proofs only on a boxed node]",
    ),
    ("rejected", "escalate", "escalated  [guard: to > rung]"),
    ("rejected", "requeue", "open  (generation + 1)"),
    ("rejected", "propose_terminal", "terminal_candidate"),
    (
        "terminal_candidate",
        "declare_terminal",
        "terminal  [reducer: count >= 1, key set, node bound unless preferred]",
    ),
    ("terminal_candidate", "reopen", "open  (generation + 1)"),
    ("retired", "reinstate", "accepted(its level)"),
    (
        "open|in_progress|produced|rejected|terminal_candidate",
        "attention",
        "attention(code)",
    ),
    ("attention", "reopen", "open  (generation + 1)"),
    (
        "attention|rejected|skipped|stale",
        "control_retry",
        "open  (generation + 1; never counted as a failure)",
    ),
    (
        "open|attention|rejected|terminal_candidate",
        "control_skip",
        "skipped",
    ),
    ("<any but stale>", "mark_stale", "stale"),
    (
        "stale",
        "reopen",
        "open  (generation + 1; node must not be stale)",
    ),
];

pub fn apply_item(
    state: &ItemState,
    ev: &ItemMachineEvent,
) -> Result<ItemState, IllegalTransition> {
    use ItemMachineEvent as E;
    use ItemPhase as P;
    let illegal = |why: &str| {
        Err(IllegalTransition::new(
            "work item",
            state.phase.tag(),
            ev.tag(),
            why,
        ))
    };
    let to = |phase: ItemPhase| {
        Ok(ItemState {
            phase,
            rung: state.rung,
        })
    };
    let same = || Ok(state.clone());
    match (&state.phase, ev) {
        (_, E::MarkStale) => to(P::Stale), // from any phase; repeat at stale is a no-op
        (
            P::Open | P::InProgress | P::Produced | P::Rejected(_) | P::TerminalCandidate,
            E::Attention(c),
        ) => to(P::Attention(c.clone())),
        (P::Attention(cur), E::Attention(c)) if c == cur => same(),
        (P::Open | P::Attention(_) | P::Rejected(_) | P::TerminalCandidate, E::ControlSkip) => {
            to(P::Skipped)
        }
        (P::Skipped, E::ControlSkip) => same(),
        (P::Attention(_) | P::Rejected(_) | P::Skipped | P::Stale, E::ControlRetry) => to(P::Open),
        (P::Open, E::Start) => to(P::InProgress),
        (P::Open, E::ProposeTerminal) => to(P::TerminalCandidate),
        (P::Open, E::Import(l)) => to(P::Accepted(*l)),
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
        (P::Accepted(cur), E::Accept(l) | E::Import(l)) if l == cur => same(),
        (P::Accepted(cur), E::Upgrade(l)) if l > cur => to(P::Accepted(*l)),
        (P::Accepted(cur), E::Upgrade(l)) if l == cur => same(),
        (P::Accepted(_), E::Upgrade(_)) => illegal("evidence level never decreases"),
        (P::Accepted(_), E::Reject(r)) => to(P::Rejected(r.clone())),
        (P::Accepted(_), E::Retire) => to(P::Retired),
        (P::Accepted(_), _) => illegal("item accepted"),
        (P::Rejected(cur), E::Reject(r)) if r == cur => same(),
        (P::Rejected(_), E::Escalate(r)) if *r > state.rung => to(P::Escalated(*r)),
        (P::Rejected(_), E::Escalate(_)) => illegal("escalation must go to a higher rung"),
        (P::Rejected(_), E::Requeue) => to(P::Open),
        (P::Rejected(_), E::ProposeTerminal) => to(P::TerminalCandidate),
        (P::Rejected(_), _) => illegal("item rejected"),
        (P::Escalated(cur), E::Escalate(r)) if r == cur => same(),
        (P::Escalated(_), _) => {
            illegal("item escalated (final; its next-rung item carries the work)")
        }
        (P::TerminalCandidate, E::ProposeTerminal) => same(),
        (P::TerminalCandidate, E::DeclareTerminal) => to(P::Terminal),
        (P::TerminalCandidate, E::Reopen) => to(P::Open),
        (P::TerminalCandidate, _) => illegal("terminal candidate"),
        (P::Terminal, E::DeclareTerminal) => same(),
        (P::Terminal, _) => illegal("terminal declared (final)"),
        (P::Retired, E::Retire) => same(),
        (P::Retired, E::Reinstate(l)) => to(P::Accepted(*l)),
        (P::Retired, _) => illegal("rule retired (only reinstate)"),
        (P::Attention(_), E::Reopen) => to(P::Open),
        (P::Attention(_), _) => illegal("item needs attention (operator retry / skip)"),
        (P::Skipped, _) => illegal("item skipped (operator retry)"),
        (P::Stale, E::Reopen) => to(P::Open),
        (P::Stale, _) => illegal("item stale (reopen first)"),
    }
}

// ------------------------------------------------------------------------------------------------------------
// (iii) NODE
// ------------------------------------------------------------------------------------------------------------
//
// R33 / R35 (user, binding): every node FIRST aims at FULL closure. Only when that attempt actually ran and ended
// within its spec budget (U-6), and only if the node's SPEC carries a user-provided box (an input of role "box",
// U-7), may it fall back to closure inside that box (BOXED); a node ever closed FULL is never boxed. A node
// without a spec box that cannot be closed fully ends OPEN with its blockers.
// Closed / certified are DERIVED verdicts (audit A C2): bound to a closure result computed on the node's CURRENT
// rule set and dispatch order, to no open item, and (certified) to every live rule at the spec's level; any event
// that breaks one of these moves the node back to running automatically (`node_reopened` with the cause).

/// What the node is currently trying to close.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "target", rename_all = "snake_case")]
pub enum ClosureTarget {
    Full,
    Boxed { box_id: String },
}

/// What prevented the closure attempt of the current target: counts and the key / face list file.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blockers {
    pub faces: u64,
    pub loci: u64,
    pub keys: u64,
    pub list: Option<FileRef>,
}

impl Blockers {
    pub fn is_empty(&self) -> bool {
        self.faces == 0 && self.loci == 0 && self.keys == 0
    }
}

/// Why the attempt on the current target ended (U-6): the spec budget that was exhausted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptEndCause {
    CpuBudget,
    FixerRounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodePhase {
    Registered,
    Running,
    ClosureCheck,
    /// Derived verdict: closure of the target reached on the current rule set, no open item.
    Closed,
    /// Closed AND every live rule at the spec's certification level.
    Certified,
    /// Ended without closure: the full attempt (and the box attempt, if the spec has a box) ended.
    Open,
    Stale,
}

impl NodePhase {
    pub fn tag(self) -> &'static str {
        match self {
            NodePhase::Registered => "registered",
            NodePhase::Running => "running",
            NodePhase::ClosureCheck => "closure_check",
            NodePhase::Closed => "closed",
            NodePhase::Certified => "certified",
            NodePhase::Open => "open",
            NodePhase::Stale => "stale",
        }
    }
    pub const ALL: [NodePhase; 7] = [
        NodePhase::Registered,
        NodePhase::Running,
        NodePhase::ClosureCheck,
        NodePhase::Closed,
        NodePhase::Certified,
        NodePhase::Open,
        NodePhase::Stale,
    ];
}

/// Default full-attempt budget (U-6): 50 CPU-hours ...
pub const DEFAULT_FULL_ATTEMPT_CPU_HOURS: u32 = 50;
/// ... or 5 fixer rounds, whichever first.
pub const DEFAULT_FULL_ATTEMPT_FIXER_ROUNDS: u32 = 5;
/// Default lowest rung of a non-preferred terminal (the ladder's terminal rung; spec-overridable).
pub const DEFAULT_TERMINAL_RUNG: Rung = Rung(5);

/// The node's spec values with defaults resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSpec {
    pub certify_full: CertLevel,
    pub certify_boxed: CertLevel,
    pub full_attempt_cpu_ms: u64,
    pub full_attempt_fixer_rounds: u32,
    pub terminal_rung: Rung,
    pub preferred_terminals: Option<FileRef>,
    pub preferred_listed: Option<u64>,
    pub preferred_count_against_bound: bool,
    /// lane closeloop3: the start target (`full` unless the spec says `boxed`; `full` is not serialized).
    #[serde(default, skip_serializing_if = "crate::ledger::StartTarget::is_full")]
    pub start_target: crate::ledger::StartTarget,
}

impl NodeSpec {
    pub fn resolve(v: &NodeSpecValues) -> NodeSpec {
        NodeSpec {
            certify_full: v.certify_full.unwrap_or(CertLevel::Replay),
            certify_boxed: v.certify_boxed.unwrap_or(CertLevel::PerKey),
            full_attempt_cpu_ms: v
                .full_attempt_cpu_hours
                .unwrap_or(DEFAULT_FULL_ATTEMPT_CPU_HOURS) as u64
                * 3_600_000,
            full_attempt_fixer_rounds: v
                .full_attempt_fixer_rounds
                .unwrap_or(DEFAULT_FULL_ATTEMPT_FIXER_ROUNDS),
            terminal_rung: v.terminal_rung.unwrap_or(DEFAULT_TERMINAL_RUNG),
            preferred_terminals: v.preferred_terminals.clone(),
            preferred_listed: v.preferred_listed,
            preferred_count_against_bound: v.preferred_count_against_bound.unwrap_or(false),
            start_target: v.start_target.unwrap_or_default(),
        }
    }
}

impl Default for NodeSpec {
    fn default() -> Self {
        NodeSpec::resolve(&NodeSpecValues::default())
    }
}

/// The closure result the node's verdict is bound to (audit A C2, R41 P-2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClosureBinding {
    pub round: u32,
    pub defect_keys: u64,
    pub closure_keys: u64,
    pub rule_set_digest: String,
    pub dispatch_order_digest: String,
    pub envelope: EnvelopeKind,
    pub envelope_tables: Vec<FileRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeState {
    pub phase: NodePhase,
    pub target: ClosureTarget,
    /// The box of the node's spec (input id), if the user provided one. Never chosen by the campaign.
    pub spec_box: Option<String>,
    pub spec: NodeSpec,
    /// The attempt on the current target ended (within its budgets) without closure.
    pub attempt_ended: bool,
    pub full_blockers: Option<Blockers>,
    pub box_blockers: Option<Blockers>,
    pub closure_rounds: u32,
    pub last_defect: Option<u64>,
    pub closure_keys: Option<u64>,
    pub rules_used: Option<u64>,
    pub rules_total: Option<u64>,
    /// Closure result on the current target (cleared when the target changes).
    pub closure: Option<ClosureBinding>,
    /// Digest of the current dispatch order (`assembled`) and the assembled table manifest.
    pub dispatch_order: Option<String>,
    pub assembled_manifest: Option<FileRef>,
    pub last_fixer_round: u32,
    pub fixer_rounds_on_target: u32,
    pub cpu_ms_on_target: u64,
    /// The node was once closed on its FULL target: it is never boxed (R33).
    pub full_closed_ever: bool,
    /// Counted terminal keys (against `terminal_bound`) and preferred terminal keys.
    pub terminals: u32,
    pub preferred_terminals: u32,
    pub terminal_bound: u32,
    pub reopens: u32,
    pub reopen_cause: Option<String>,
}

impl NodeState {
    pub fn new(spec_box: Option<String>, terminal_bound: u32, spec: NodeSpec) -> Self {
        // lane closeloop3: a node registered `start_target: boxed` is BOXED from its first record (the registration
        // refuses it without a spec box)
        let target = start_target_of(&spec, &spec_box);
        NodeState {
            phase: NodePhase::Registered,
            target,
            spec_box,
            spec,
            attempt_ended: false,
            full_blockers: None,
            box_blockers: None,
            closure_rounds: 0,
            last_defect: None,
            closure_keys: None,
            rules_used: None,
            rules_total: None,
            closure: None,
            dispatch_order: None,
            assembled_manifest: None,
            last_fixer_round: 0,
            fixer_rounds_on_target: 0,
            cpu_ms_on_target: 0,
            full_closed_ever: false,
            terminals: 0,
            preferred_terminals: 0,
            terminal_bound,
            reopens: 0,
            reopen_cause: None,
        }
    }
    /// FULL / BOXED / OPEN as shown on the dashboard.
    pub fn closure_kind(&self) -> &'static str {
        match (self.phase, &self.target) {
            (NodePhase::Closed | NodePhase::Certified, ClosureTarget::Full) => "full",
            (NodePhase::Closed | NodePhase::Certified, ClosureTarget::Boxed { .. }) => "boxed",
            _ => "open",
        }
    }
    pub fn certified(&self) -> bool {
        self.phase == NodePhase::Certified
    }
    /// Closed against MEASURED hand-down ranges, not yet rechecked against proved ones (R41 P-2).
    pub fn provisional(&self) -> bool {
        matches!(self.phase, NodePhase::Closed | NodePhase::Certified)
            && self
                .closure
                .as_ref()
                .is_some_and(|c| c.envelope == EnvelopeKind::Measured)
    }
    /// The certification level the spec asks for the current target.
    pub fn required_cert(&self) -> CertLevel {
        match self.target {
            ClosureTarget::Full => self.spec.certify_full,
            ClosureTarget::Boxed { .. } => self.spec.certify_boxed,
        }
    }
}

/// lane closeloop3: the target a node starts on (and returns to on a reopen): its spec box when registered
/// `start_target: boxed`, else FULL.
pub fn start_target_of(spec: &NodeSpec, spec_box: &Option<String>) -> ClosureTarget {
    match (spec.start_target, spec_box) {
        (crate::ledger::StartTarget::Boxed, Some(b)) => ClosureTarget::Boxed { box_id: b.clone() },
        _ => ClosureTarget::Full,
    }
}

/// Context of a node transition supplied by the reducer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NodeContext {
    pub items_open: u64,
    /// Live accepted rules below `identity_tested` / below `identity_proved`.
    pub below_tested: u64,
    pub below_proved: u64,
    /// Digest of the node's current live rule set ([`rule_set_digest_of`]).
    pub rule_set_digest: String,
}

impl NodeContext {
    fn below(&self, c: CertLevel) -> u64 {
        match c {
            CertLevel::Replay => self.below_tested,
            CertLevel::PerKey => self.below_proved,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum NodeEvent {
    Start,
    /// A fixer round begins (plan `fixer_round_begin`): counts against the full-attempt budget.
    FixerRoundBegin {
        round: u32,
        partition: Option<FileRef>,
    },
    /// The node's table set was assembled (plan `assembled` + `dispatch_order_fixed`): fixes the dispatch order.
    Assembled {
        round: u32,
        manifest: FileRef,
        dispatch_order: FileRef,
    },
    /// A closure pass of the current target, computed on the rule set `rule_set_digest` under the dispatch order
    /// `dispatch_order_digest` (both must be the node's current ones) against hand-down ranges `envelope`.
    ClosureResult {
        round: u32,
        defect_keys: u64,
        closure_keys: u64,
        rules_used: Option<u64>,
        rules_total: Option<u64>,
        keyset: Option<FileRef>,
        rule_set_digest: String,
        dispatch_order_digest: String,
        envelope: EnvelopeKind,
        envelope_tables: Vec<FileRef>,
    },
    /// Informational (plan `winner_diff_done`).
    WinnerDiffDone {
        round: u32,
        diff: FileRef,
        failures: u64,
    },
    /// The attempt on the current target ended: it ran (a closure result with defect > 0 exists) and the spec
    /// budget named by `cause` is exhausted.
    AttemptEnded {
        blockers: Blockers,
        reason: String,
        cause: AttemptEndCause,
    },
    /// Fall back from FULL to the spec's box.
    FallBackToBox {
        box_id: String,
    },
    DeclareOpen {
        reason: String,
    },
    /// Plan `table_final` (the closure binding names the table set).
    DeclareClosed,
    DeclareCertified,
    /// Keys-only recheck of a provisional closure against PROVED hand-down ranges (R41 P-2): passed clears
    /// `provisional`, failed reopens the node.
    EnvelopeRecheck {
        passed: bool,
        report: FileRef,
        envelope_tables: Vec<FileRef>,
    },
    /// Back to work; the target is FULL again (a later run tries full closure first).
    Reopen {
        reason: String,
    },
    /// Plan `verdict_stale`.
    MarkStale {
        reason: String,
    },
}

impl NodeEvent {
    pub fn tag(&self) -> &'static str {
        match self {
            NodeEvent::Start => "start",
            NodeEvent::FixerRoundBegin { .. } => "fixer_round_begin",
            NodeEvent::Assembled { .. } => "assembled",
            NodeEvent::ClosureResult { .. } => "closure_result",
            NodeEvent::WinnerDiffDone { .. } => "winner_diff_done",
            NodeEvent::AttemptEnded { .. } => "attempt_ended",
            NodeEvent::FallBackToBox { .. } => "fall_back_to_box",
            NodeEvent::DeclareOpen { .. } => "declare_open",
            NodeEvent::DeclareClosed => "declare_closed",
            NodeEvent::DeclareCertified => "declare_certified",
            NodeEvent::EnvelopeRecheck { .. } => "envelope_recheck",
            NodeEvent::Reopen { .. } => "reopen",
            NodeEvent::MarkStale { .. } => "mark_stale",
        }
    }
}

/// Node transitions caused by the reducer itself (never records: audit B exp1, an internal event could be
/// appended and panic the writer).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeInternal {
    /// `count` counted terminal keys declared.
    TerminalDeclared { count: u32 },
    /// A rule retirement; `box_only` = the proof is a box closure pass.
    Retirement { box_only: bool },
}

pub const NODE_TABLE: &[(&str, &str, &str)] = &[
    ("registered", "start", "running (target full)"),
    (
        "running|closure_check",
        "fixer_round_begin",
        "same phase, fixer rounds on target + 1  [guard: round > last]",
    ),
    (
        "running|closure_check|closed|certified|open",
        "assembled",
        "same phase, dispatch order fixed (a change reopens a closed node)",
    ),
    (
        "running|closure_check",
        "closure_result",
        "closure_check  [guards: round > last; rule set and dispatch order = current]",
    ),
    (
        "closed|certified",
        "closure_result",
        "same phase if defect = 0, else closure_check  [same guards]",
    ),
    (
        "running|closure_check|closed|certified|open",
        "winner_diff_done",
        "same phase",
    ),
    (
        "running|closure_check",
        "attempt_ended",
        "same phase, attempt_ended, blockers  [guards: closure result on target with defect > 0, blockers, spec budget exhausted]",
    ),
    (
        "running|closure_check",
        "fall_back_to_box",
        "running, target boxed  [guards: target full, full attempt ended, never closed full, spec box = box_id]",
    ),
    (
        "running|closure_check",
        "declare_open",
        "open  [guards: attempt ended; target boxed, or no spec box]",
    ),
    (
        "closure_check",
        "declare_closed",
        "closed  [guards: bound closure result with defect 0 on the CURRENT rule set and dispatch order, no open item]",
    ),
    (
        "closed",
        "declare_certified",
        "certified  [guard: every live rule at the spec's level (FULL: replay, BOXED: per-key by default)]",
    ),
    (
        "closed|certified",
        "envelope_recheck",
        "same phase, provisional cleared (passed) / running (failed)  [guard: provisional]",
    ),
    (
        "running|closure_check|closed|certified|open|stale",
        "reopen",
        "running, target full, attempt not ended (running on FULL: no-op)",
    ),
    ("<any>", "mark_stale", "stale"),
    (
        "<derived>",
        "node_reopened",
        "closed|certified -> running when the rule set, dispatch order, open items or levels change",
    ),
];

pub fn apply_node(
    state: &NodeState,
    ev: &NodeEvent,
    ctx: &NodeContext,
) -> Result<NodeState, IllegalTransition> {
    use NodePhase as P;
    let illegal = |why: String| {
        Err(IllegalTransition::new(
            "node",
            state.phase.tag(),
            ev.tag(),
            why,
        ))
    };
    let mut next = state.clone();
    match (state.phase, ev) {
        (_, NodeEvent::MarkStale { .. }) => next.phase = P::Stale,
        (P::Registered, NodeEvent::Start) => next.phase = P::Running,
        (P::Running, NodeEvent::Start) => {}
        (_, NodeEvent::Start) => return illegal("node already started".into()),
        (P::Running | P::ClosureCheck, NodeEvent::FixerRoundBegin { round, .. }) => {
            if *round == state.last_fixer_round && *round > 0 {
                return Ok(next); // repeat
            }
            if *round <= state.last_fixer_round {
                return illegal(format!(
                    "fixer round {round} not after round {}",
                    state.last_fixer_round
                ));
            }
            next.last_fixer_round = *round;
            next.fixer_rounds_on_target += 1;
        }
        (_, NodeEvent::FixerRoundBegin { .. }) => {
            return illegal("fixer rounds need a running node".into());
        }
        (
            P::Running | P::ClosureCheck | P::Closed | P::Certified | P::Open,
            NodeEvent::Assembled {
                manifest,
                dispatch_order,
                ..
            },
        ) => {
            next.dispatch_order = Some(dispatch_order.blake3.clone());
            next.assembled_manifest = Some(manifest.clone());
        }
        (_, NodeEvent::Assembled { .. }) => return illegal("node not started or stale".into()),
        (
            P::Running | P::ClosureCheck | P::Closed | P::Certified | P::Open,
            NodeEvent::WinnerDiffDone { .. },
        ) => {}
        (_, NodeEvent::WinnerDiffDone { .. }) => {
            return illegal("node not started or stale".into());
        }
        (P::Registered | P::Stale | P::Open, NodeEvent::ClosureResult { .. }) => {
            return illegal("closure results need a running node".into());
        }
        (
            phase,
            NodeEvent::ClosureResult {
                round,
                defect_keys,
                closure_keys,
                rules_used,
                rules_total,
                rule_set_digest,
                dispatch_order_digest,
                envelope,
                envelope_tables,
                ..
            },
        ) => {
            let binding = ClosureBinding {
                round: *round,
                defect_keys: *defect_keys,
                closure_keys: *closure_keys,
                rule_set_digest: rule_set_digest.clone(),
                dispatch_order_digest: dispatch_order_digest.clone(),
                envelope: *envelope,
                envelope_tables: envelope_tables.clone(),
            };
            if state.closure.as_ref() == Some(&binding) {
                return Ok(next); // exact repeat
            }
            if *round <= state.closure_rounds {
                return illegal(format!(
                    "closure round {round} not after round {}",
                    state.closure_rounds
                ));
            }
            if let (Some(u), Some(t)) = (rules_used, rules_total)
                && u > t
            {
                return illegal(format!("rules used {u} > rules total {t}"));
            }
            if *rule_set_digest != ctx.rule_set_digest {
                return illegal(format!(
                    "closure computed on rule set {} but the node's current rule set is {}",
                    short(rule_set_digest),
                    short(&ctx.rule_set_digest)
                ));
            }
            match &state.dispatch_order {
                Some(d) if d == dispatch_order_digest => {}
                Some(d) => {
                    return illegal(format!(
                        "closure computed under dispatch order {} but the current one is {}",
                        short(dispatch_order_digest),
                        short(d)
                    ));
                }
                None => {
                    return illegal("no dispatch order fixed (assembled) for this node".into());
                }
            }
            next.closure_rounds = *round;
            next.last_defect = Some(*defect_keys);
            next.closure_keys = Some(*closure_keys);
            next.rules_used = *rules_used;
            next.rules_total = *rules_total;
            next.closure = Some(binding);
            next.phase = match phase {
                P::Closed | P::Certified if *defect_keys == 0 => phase,
                _ => P::ClosureCheck,
            };
        }
        (
            P::Running | P::ClosureCheck,
            NodeEvent::AttemptEnded {
                blockers, cause, ..
            },
        ) => {
            if state.attempt_ended {
                let same = match state.target {
                    ClosureTarget::Full => state.full_blockers.as_ref() == Some(blockers),
                    ClosureTarget::Boxed { .. } => state.box_blockers.as_ref() == Some(blockers),
                };
                if same {
                    return Ok(next);
                }
                return illegal("attempt already ended with other blockers".into());
            }
            match &state.closure {
                Some(c) if c.defect_keys > 0 => {}
                Some(_) => {
                    return illegal("the last closure result has defect 0: close, not end".into());
                }
                None => {
                    return illegal(
                        "no closure result on the current target: the attempt has not run".into(),
                    );
                }
            }
            if blockers.is_empty() {
                return illegal("an ended attempt names what blocked it".into());
            }
            match cause {
                AttemptEndCause::CpuBudget
                    if state.cpu_ms_on_target < state.spec.full_attempt_cpu_ms =>
                {
                    return illegal(format!(
                        "CPU budget not exhausted: {} of {} ms on this target",
                        state.cpu_ms_on_target, state.spec.full_attempt_cpu_ms
                    ));
                }
                AttemptEndCause::FixerRounds
                    if state.fixer_rounds_on_target < state.spec.full_attempt_fixer_rounds =>
                {
                    return illegal(format!(
                        "fixer-round budget not exhausted: {} of {} rounds on this target",
                        state.fixer_rounds_on_target, state.spec.full_attempt_fixer_rounds
                    ));
                }
                _ => {}
            }
            next.attempt_ended = true;
            match state.target {
                ClosureTarget::Full => next.full_blockers = Some(blockers.clone()),
                ClosureTarget::Boxed { .. } => next.box_blockers = Some(blockers.clone()),
            }
        }
        (_, NodeEvent::AttemptEnded { .. }) => return illegal("no attempt running".into()),
        (P::Running | P::ClosureCheck, NodeEvent::FallBackToBox { box_id }) => {
            if let ClosureTarget::Boxed { box_id: cur } = &state.target {
                if cur == box_id {
                    return Ok(next); // repeat
                }
                return illegal("already boxed with another box".into());
            }
            if state.full_closed_ever {
                return illegal("the node was closed FULL once: it is never boxed (R33)".into());
            }
            match &state.spec_box {
                None => {
                    return illegal(
                        "BOXED needs a box in the node's spec (the campaign never chooses one)"
                            .into(),
                    );
                }
                Some(b) if b != box_id => {
                    return illegal(format!(
                        "box {box_id} is not the spec box {b} (never chosen or widened)"
                    ));
                }
                Some(_) => {}
            }
            if !state.attempt_ended {
                return illegal(
                    "the FULL attempt must run and end (within its budgets) before falling back to the box"
                        .into(),
                );
            }
            next.target = ClosureTarget::Boxed {
                box_id: box_id.clone(),
            };
            next.attempt_ended = false;
            next.phase = P::Running;
            next.last_defect = None;
            next.closure = None;
            next.cpu_ms_on_target = 0;
            next.fixer_rounds_on_target = 0;
        }
        (_, NodeEvent::FallBackToBox { .. }) => {
            return illegal("fallback needs a running attempt".into());
        }
        (P::Open, NodeEvent::DeclareOpen { .. }) => {}
        (P::Running | P::ClosureCheck, NodeEvent::DeclareOpen { .. }) => {
            if !state.attempt_ended {
                return illegal("the attempt has not ended".into());
            }
            if state.target == ClosureTarget::Full && state.spec_box.is_some() {
                return illegal(
                    "the spec provides a box: fall back to it before declaring the node open"
                        .into(),
                );
            }
            next.phase = P::Open;
        }
        (_, NodeEvent::DeclareOpen { .. }) => return illegal("nothing to declare open".into()),
        (P::ClosureCheck, NodeEvent::DeclareClosed) => {
            let Some(c) = &state.closure else {
                return illegal("no closure result".into());
            };
            if c.defect_keys != 0 {
                return illegal(format!("closure defect is {}, not 0", c.defect_keys));
            }
            if c.rule_set_digest != ctx.rule_set_digest {
                return illegal("the closure result is of an older rule set".into());
            }
            if state.dispatch_order.as_deref() != Some(c.dispatch_order_digest.as_str()) {
                return illegal("the closure result is of an older dispatch order".into());
            }
            if ctx.items_open > 0 {
                return illegal(format!("{} items still open", ctx.items_open));
            }
            next.phase = P::Closed;
            next.reopen_cause = None;
            if state.target == ClosureTarget::Full {
                next.full_closed_ever = true;
            }
        }
        (P::Closed | P::Certified, NodeEvent::DeclareClosed) => {}
        (_, NodeEvent::DeclareClosed) => {
            return illegal("needs a closure check with defect 0".into());
        }
        (P::Closed, NodeEvent::DeclareCertified) => {
            let need = state.required_cert();
            let below = ctx.below(need);
            if below > 0 {
                return illegal(format!(
                    "{below} live rules are below the spec's certification level {}",
                    need.word()
                ));
            }
            next.phase = P::Certified;
        }
        (P::Certified, NodeEvent::DeclareCertified) => {}
        (_, NodeEvent::DeclareCertified) => return illegal("needs closed first".into()),
        (
            P::Closed | P::Certified,
            NodeEvent::EnvelopeRecheck {
                passed,
                envelope_tables,
                ..
            },
        ) => {
            if !state.provisional() {
                return illegal("the closure is not provisional".into());
            }
            if *passed {
                if let Some(c) = next.closure.as_mut() {
                    c.envelope = EnvelopeKind::Proved;
                    c.envelope_tables = envelope_tables.clone();
                }
            } else {
                next.phase = P::Running;
                next.reopens += 1;
                next.reopen_cause =
                    Some("keys-only recheck against proved hand-down ranges failed".into());
            }
        }
        (_, NodeEvent::EnvelopeRecheck { .. }) => {
            return illegal("only a closed node is rechecked".into());
        }
        (P::Running, NodeEvent::Reopen { .. })
            if state.target == start_target_of(&state.spec, &state.spec_box) => {}
        (
            P::Running | P::ClosureCheck | P::Closed | P::Certified | P::Open | P::Stale,
            NodeEvent::Reopen { reason },
        ) => {
            next.phase = P::Running;
            // lane closeloop3: back to the START target (FULL, or the spec box of a node registered boxed: it never
            // had a FULL target)
            let start = start_target_of(&state.spec, &state.spec_box);
            if state.target != start {
                next.cpu_ms_on_target = 0;
                next.fixer_rounds_on_target = 0;
                next.closure = None;
            }
            next.target = start;
            next.attempt_ended = false;
            next.reopens += 1;
            next.reopen_cause = Some(reason.clone());
        }
        (P::Registered, NodeEvent::Reopen { .. }) => return illegal("node not started".into()),
    }
    Ok(next)
}

/// Reducer-internal node transitions.
pub fn apply_node_internal(
    state: &NodeState,
    ev: NodeInternal,
) -> Result<NodeState, IllegalTransition> {
    let tag = match ev {
        NodeInternal::TerminalDeclared { .. } => "terminal_declared",
        NodeInternal::Retirement { .. } => "retirement",
    };
    let illegal = |why: String| Err(IllegalTransition::new("node", state.phase.tag(), tag, why));
    if state.phase == NodePhase::Stale {
        return illegal("node is stale".into());
    }
    let mut next = state.clone();
    match ev {
        NodeInternal::TerminalDeclared { count } => {
            let total = state.terminals.saturating_add(count);
            if total > state.terminal_bound {
                return illegal(format!(
                    "terminal bound exceeded: {total} > {} (the spec's bound)",
                    state.terminal_bound
                ));
            }
            next.terminals = total;
        }
        NodeInternal::Retirement { box_only } => {
            if box_only && state.target == ClosureTarget::Full {
                return illegal(
                    "a box closure pass cannot retire a rule of a FULL node (no coverage removal; R33)"
                        .into(),
                );
            }
        }
    }
    Ok(next)
}

// ------------------------------------------------------------------------------------------------------------
// Rule-set digest (the identity a closure result is bound to)
// ------------------------------------------------------------------------------------------------------------

/// Order-independent accumulator of a node's live rule set: wrapping sum of per-rule BLAKE3 words + count.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleSetAcc {
    pub sum: [u64; 4],
    pub count: u64,
}

/// The element of one live rule: (item, rule id, accepted record digest, the SET of key-exclusion digests).
pub fn rule_element(
    item: &str,
    rule: &str,
    record_blake3: &str,
    exclusions: &[String],
) -> [u64; 4] {
    let mut h = blake3::Hasher::new();
    for part in [item, rule, record_blake3] {
        h.update(part.as_bytes());
        h.update(&[0]);
    }
    // audit-ledger2 m3: the exclusions as a SET (sorted), so the same live rule set has one digest whatever the
    // order of the exclusion records (identical bytes to the earlier form for 0 or 1 exclusion)
    let mut sorted: Vec<&String> = exclusions.iter().collect();
    sorted.sort_unstable();
    for e in sorted {
        h.update(e.as_bytes());
        h.update(&[1]);
    }
    let b = h.finalize();
    let b = b.as_bytes();
    let mut w = [0u64; 4];
    for (i, x) in w.iter_mut().enumerate() {
        *x = u64::from_le_bytes(b[i * 8..i * 8 + 8].try_into().unwrap());
    }
    w
}

impl RuleSetAcc {
    pub fn add(&mut self, e: [u64; 4]) {
        for (s, x) in self.sum.iter_mut().zip(e) {
            *s = s.wrapping_add(x);
        }
        self.count += 1;
    }
    pub fn sub(&mut self, e: [u64; 4]) {
        for (s, x) in self.sum.iter_mut().zip(e) {
            *s = s.wrapping_sub(x);
        }
        self.count -= 1;
    }
    pub fn digest(&self) -> String {
        let mut h = blake3::Hasher::new();
        h.update(b"rustred.campaign.rule-set.v1\0");
        h.update(&self.count.to_le_bytes());
        for s in self.sum {
            h.update(&s.to_le_bytes());
        }
        h.finalize().to_hex().to_string()
    }
}

/// The rule-set digest of a list of live rules `(item, rule, record_blake3, exclusions)`: what a closure tool
/// computes on the table set it ran on (equal to the reducer's incremental value for the same set).
pub fn rule_set_digest_of<'a>(
    rules: impl IntoIterator<Item = (&'a str, &'a str, &'a str, &'a [String])>,
) -> String {
    let mut acc = RuleSetAcc::default();
    for (i, r, b, x) in rules {
        acc.add(rule_element(i, r, b, x));
    }
    acc.digest()
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
    (
        "starting|running|draining",
        "start",
        "starting  (previous supervisor died: crash recovery)",
    ),
    ("starting", "run", "running"),
    ("starting|running", "drain", "draining"),
    ("starting|running|draining", "stop", "stopped"),
    (
        "running",
        "finish",
        "finished  [reducer: no launching / running attempt, no pending intent]",
    ),
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

/// Provenance of hand-made evidence or an imported table set (both cap the level of what comes from them).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProvenanceRec {
    /// `evidence_imported` or `tables_imported`.
    pub kind: String,
    pub producer: String,
    pub description: String,
    pub source_campaign: Option<String>,
    pub manifest: Option<FileRef>,
    pub files: Vec<FileRef>,
    pub level_claimed: EvidenceLevel,
    /// False for evidence whose producer digest is unknown (provenance only: no evidence level from it by itself).
    pub producer_known: bool,
    /// The provenance class stated for an unknown producer.
    pub provenance_class: Option<String>,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeRec {
    pub state: NodeState,
    pub inputs: Vec<String>,
    /// Items of the node per phase, indexed like [`ItemPhase::TAGS`].
    pub items_by_phase: [u64; N_ITEM_PHASES],
    /// Live accepted items per evidence level (indexed like [`EvidenceLevel::ALL`]).
    pub accepted_by_level: [u64; 3],
    pub rule_set: RuleSetAcc,
    pub last_keyset: Option<FileRef>,
    pub stale_reason: Option<String>,
    pub cpu_ms: u64,
    pub paused: bool,
    /// Comparison-only numbers (U-9): never read by any check.
    pub reference: Option<BTreeMap<String, String>>,
}

impl NodeRec {
    pub fn context(&self) -> NodeContext {
        let open: u64 = ItemPhase::TAGS
            .iter()
            .enumerate()
            .filter(|(_, t)| !matches!(**t, "accepted" | "escalated" | "terminal" | "retired"))
            .map(|(i, _)| self.items_by_phase[i])
            .sum();
        NodeContext {
            items_open: open,
            below_tested: self.accepted_by_level[0],
            below_proved: self.accepted_by_level[0] + self.accepted_by_level[1],
            rule_set_digest: self.rule_set.digest(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProducedRec {
    pub job: String,
    pub attempt: u32,
    pub table: FileRef,
    pub seq: u64,
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
    /// Bumped on every re-entry into `open`: results only from jobs queued in the current generation.
    pub generation: u32,
    pub jobs: Vec<String>,
    /// Items created with this item as `parent` (the parent -> children index: staleness, reopen and retry find
    /// the children in O(children), never by a scan of every item; audit-ledger2 M4).
    pub children: Vec<String>,
    // The sub-records below are boxed: most items never carry most of them, and a boxed `None` is one word.
    pub produced: Option<Box<ProducedRec>>,
    pub accepted: Option<Box<AcceptedRec>>,
    pub economy: Option<Box<RuleEconomy>>,
    pub retired: Option<Box<RetiredRec>>,
    /// Digests of key-exclusion artefacts of the accepted rule (part of its rule-set element).
    pub exclusions: Vec<String>,
    pub stale_from: Option<String>,
    pub stale_input: Option<String>,
    pub attention: Option<String>,
    pub terminal_keys: u32,
    pub terminal_preferred: bool,
    /// The terminal keys count against the node bound.
    pub terminal_counted: bool,
    pub terminal_keyset: Option<Box<FileRef>>,
    pub ladder_exhausted: Option<Box<FileRef>>,
    pub paused: bool,
    /// Attempts of this item's jobs that ended `failed` (interrupts and losses never count, U-5).
    pub failed_attempts: u32,
    pub operator_retries: u32,
    pub cpu_ms: u64,
    pub created_seq: u64,
    pub updated_seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum AcceptedFrom {
    Job {
        job: String,
        attempt: u32,
        table_blake3: String,
    },
    Evidence {
        evidence: String,
    },
    Import {
        provenance: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptedRec {
    pub rule: String,
    pub level: EvidenceLevel,
    pub record: FileRef,
    pub source: AcceptedFrom,
    pub round: Option<u32>,
    pub seq: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetiredRec {
    pub proof: RetirementProof,
    pub superseded_by: Option<String>,
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
    /// Generation of the item when the job was queued.
    pub generation: u32,
    /// Input identity: the ids of every input the job depends on, and one digest over (id, input digest) of all of
    /// them at queue time ([`stamp_digest`]).
    pub inputs: Vec<String>,
    pub inputs_digest: String,
    /// Set (never cleared) when one of those inputs changed: the job can feed no result.
    pub stale: Option<String>,
    pub attempt: u32,
    pub state: AttemptState,
    pub accept: Option<ContractAccept>,
    pub run_dir: Option<String>,
    pub pid: Option<u32>,
    pub start_ticks: Option<u64>,
    pub boot_id: Option<String>,
    pub sealed_table: Option<FileRef>,
    pub outcome: Option<FileRef>,
    /// `spec_blake3` of the CURRENT attempt's launch record (its job.json digest): the OUTCOME binding must name it.
    pub attempt_spec: Option<String>,
    /// The same of the PREVIOUS attempt (kept across a new attempt: a resume reads that attempt's run directory).
    pub prev_attempt_spec: Option<String>,
    pub time_decided: bool,
    /// Session number (`campaign start` count) in which the CURRENT attempt's launch intent was journalled: an
    /// attempt still `launching` in a later session is an unconfirmed launch ([`CampaignState::unconfirmed_launches`]).
    pub launch_session: Option<u32>,
    /// Attempts that ended `failed` (retry caps count these only).
    pub failures: u32,
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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlRec {
    pub verb: ControlVerb,
    pub target: ControlTarget,
    pub by: String,
    pub reason: String,
    pub requested_seq: u64,
    pub requested_utc: String,
    /// `requested` / `applied` / `refused`.
    pub status: String,
    pub outcome: Option<String>,
    pub done_seq: Option<u64>,
    pub done_utc: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleCounters {
    pub accepted: u64,
    pub imported: u64,
    pub rejected: u64,
    pub rejected_by_reason: BTreeMap<String, u64>,
    pub upgrades: u64,
    pub retired: u64,
    pub replaced: u64,
}

/// Per-node economy before / after retirements and supersessions.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeEconomy {
    pub rules_before: u64,
    pub rules_after: u64,
    pub superseded: u64,
    pub unmeasured: u64,
    pub before: RuleEconomy,
    pub after: RuleEconomy,
}

/// Per-node terminal report (R41 P-4).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TerminalReport {
    pub preferred_used: u64,
    pub preferred_listed: Option<u64>,
    pub preferred_not_needed: Option<u64>,
    pub other: u64,
    pub counted: u64,
    pub bound: u32,
}

/// Bounded length of the recent-events ring kept in the state (and in the recent-events progress file).
pub const RECENT_CAP: usize = 64;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CampaignState {
    pub campaign_id: String,
    pub spec_blake3: String,
    pub created_utc: String,
    pub terminal_bound: u32,
    pub mode: CampaignMode,
    pub phase: Option<CampaignPhase>,
    pub paused: bool,
    pub last_seq: u64,
    pub last_utc: String,
    pub sessions: u32,
    pub crash_recoveries: u32,
    pub takeovers: u32,
    pub torn_tail_recoveries: u32,
    pub forced_recoveries: u32,
    /// `saved_as` of every recovery record (crash-window check of the recovery marker).
    pub recoveries: Vec<String>,
    pub segments_opened: u32,
    pub orphans_killed: u32,
    pub last_verify: Option<(u64, bool)>,
    pub exports: BTreeMap<String, String>,
    pub inputs: BTreeMap<String, InputRec>,
    pub provenance: BTreeMap<String, ProvenanceRec>,
    pub nodes: BTreeMap<String, NodeRec>,
    /// Copy-on-write maps (cheap freeze for the background snapshot; read API of a `BTreeMap`).
    pub items: CowMap<ItemRec>,
    pub jobs: CowMap<JobRec>,
    pub pending_ops: BTreeMap<String, PendingOp>,
    pub done_ops: BTreeSet<String>,
    pub controls: BTreeMap<String, ControlRec>,
    pub rules: RuleCounters,
    pub terminals_total: u64,
    /// CPU milliseconds of every settled attempt per rung (the ETA's samples).
    pub costs_ms: BTreeMap<Rung, Vec<u64>>,
    pub cpu_ms_total: u64,
    pub progress_records: u64,
    pub recent: VecDeque<RecentEvent>,
    /// Strict recovery of unconfirmed launches ACROSS sessions (also on when [`ENFORCE_UNCONFIRMED_ACROSS_SESSIONS`]):
    /// an attempt still `launching` from an earlier session may then only be marked `unconfirmed`, confirmed late
    /// (`started`) or finished with its authenticated OUTCOME; `interrupt`, `lose` and an unauthenticated `finished`
    /// are refused. Not serialised (a reducer setting, not state).
    #[serde(skip)]
    pub strict_unconfirmed: bool,
}

/// Whether the reducer refuses the legacy recovery of an unconfirmed launch (lose / interrupt / an unauthenticated
/// finish of an attempt launched in an earlier session). ON since lane int5 (2026-11-14): the supervisor's recovery
/// marks every such attempt `unconfirmed` first and leaves it only by a late `started` (a marked process of it was
/// found), its authenticated OUTCOME, `not_spawned` (proven: authenticated run directory without the spawn sentinel),
/// or an operator's `control retry` + `cancel` (`rr_campaign control CDIR resolve JOB`).
pub const ENFORCE_UNCONFIRMED_ACROSS_SESSIONS: bool = true;

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
pub fn reduce<'a>(
    records: impl IntoIterator<Item = &'a Record>,
) -> Result<CampaignState, ReduceError> {
    let mut st = CampaignState::default();
    for r in records {
        st.apply_record(r)?;
    }
    Ok(st)
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
}

fn elem_of(id: &str, it: &ItemRec) -> Option<[u64; 4]> {
    it.accepted
        .as_ref()
        .map(|a| rule_element(id, &a.rule, &a.record.blake3, &it.exclusions))
}

impl CampaignState {
    /// Applies one record. All checks happen before any mutation: on `Err` the state is unchanged.
    pub fn apply_record(&mut self, rec: &Record) -> Result<(), ReduceError> {
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let illegal = |err: IllegalTransition| ReduceError::Illegal { seq, err };
        if self.last_seq != 0 && seq != self.last_seq + 1 {
            return Err(invalid(format!(
                "sequence gap: expected {}",
                self.last_seq + 1
            )));
        }
        if self.last_seq == 0 && !matches!(rec.event, Event::CampaignCreated { .. }) {
            return Err(invalid("the first record must be campaign_created".into()));
        }
        match &rec.event {
            Event::CampaignCreated {
                campaign_id,
                spec_blake3,
                terminal_bound,
                mode,
            } => {
                if self.last_seq != 0 {
                    return Err(invalid("campaign_created after the first record".into()));
                }
                self.campaign_id = campaign_id.clone();
                self.spec_blake3 = spec_blake3.clone();
                self.created_utc = rec.utc.clone();
                self.terminal_bound = *terminal_bound;
                self.mode = *mode;
            }
            Event::Campaign { ev, reason } => {
                if *ev == CampaignEvent::Finish && self.phase != Some(CampaignPhase::Finished) {
                    let live = self.unsettled_jobs().len();
                    if live > 0 || !self.pending_ops.is_empty() {
                        return Err(invalid(format!(
                            "finish refused: {live} attempts launching / running, {} pending intents",
                            self.pending_ops.len()
                        )));
                    }
                }
                let crash = *ev == CampaignEvent::Start
                    && matches!(
                        self.phase,
                        Some(
                            CampaignPhase::Starting
                                | CampaignPhase::Running
                                | CampaignPhase::Draining
                        )
                    );
                let next = apply_campaign(self.phase, *ev).map_err(illegal)?;
                if *ev == CampaignEvent::Start {
                    self.sessions += 1;
                    if crash {
                        self.crash_recoveries += 1;
                    }
                }
                if Some(next) != self.phase {
                    self.push_recent(
                        rec,
                        "campaign",
                        format!("campaign {} ({reason})", next.tag()),
                    );
                }
                self.phase = Some(next);
            }
            Event::SupervisorStarted { .. } => {}
            Event::LockTakeover {
                previous,
                reason,
                orphans,
            } => {
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
            Event::Recovery {
                truncated_bytes,
                last_good_seq,
                saved_as,
                forced,
                ..
            } => {
                self.torn_tail_recoveries += 1;
                if *forced {
                    self.forced_recoveries += 1;
                }
                if let Some(s) = saved_as {
                    self.recoveries.push(s.clone());
                }
                self.push_recent(
                    rec,
                    "recovery",
                    format!(
                        "{}: {truncated_bytes} bytes dropped after record {last_good_seq}",
                        if *forced {
                            "operator truncation override"
                        } else {
                            "torn ledger tail"
                        }
                    ),
                );
            }
            Event::SegmentOpened { .. } => self.segments_opened += 1,
            Event::InputRegistered {
                input,
                role,
                path,
                bytes,
                blake3,
            } => self.apply_input(rec, input, role, path, *bytes, blake3),
            Event::EvidenceImported {
                evidence,
                producer,
                description,
                files,
                level_claimed,
                producer_blake3,
                provenance_class,
            } => {
                match (producer_blake3, provenance_class.as_deref()) {
                    (None, None) => {
                        return Err(ReduceError::Invalid {
                            seq,
                            why: format!(
                                "evidence {evidence}: no producer digest and no provenance class (one of {:?})",
                                &crate::ledger::PROVENANCE_CLASSES[1..]
                            ),
                        });
                    }
                    (None, Some("recorded")) => {
                        return Err(ReduceError::Invalid {
                            seq,
                            why: format!(
                                "evidence {evidence}: provenance class `recorded` needs the producer digest"
                            ),
                        });
                    }
                    (_, Some(c)) if !crate::ledger::PROVENANCE_CLASSES.contains(&c) => {
                        return Err(ReduceError::Invalid {
                            seq,
                            why: format!("evidence {evidence}: unknown provenance class {c:?}"),
                        });
                    }
                    _ => {}
                }
                let p = ProvenanceRec {
                    kind: "evidence_imported".into(),
                    producer: producer.clone(),
                    description: description.clone(),
                    source_campaign: None,
                    manifest: None,
                    files: files.clone(),
                    level_claimed: *level_claimed,
                    producer_known: producer_blake3.is_some(),
                    provenance_class: provenance_class.clone(),
                    seq,
                };
                self.apply_provenance(rec, evidence, p)?;
            }
            Event::TablesImported {
                provenance,
                producer,
                source_campaign,
                manifest,
                files,
                level_claimed,
            } => {
                let p = ProvenanceRec {
                    kind: "tables_imported".into(),
                    producer: producer.clone(),
                    description: String::new(),
                    source_campaign: source_campaign.clone(),
                    manifest: Some(manifest.clone()),
                    files: files.clone(),
                    level_claimed: *level_claimed,
                    producer_known: true,
                    provenance_class: None,
                    seq,
                };
                self.apply_provenance(rec, provenance, p)?;
            }
            Event::NodeRegistered {
                node,
                inputs,
                terminal_bound,
                spec_box,
                spec,
                reference,
            } => {
                if let Some(n) = self.nodes.get(node) {
                    if n.inputs != *inputs
                        || n.state.spec_box != *spec_box
                        || n.state.spec != NodeSpec::resolve(spec)
                        || n.state.terminal_bound != terminal_bound.unwrap_or(self.terminal_bound)
                        || n.reference != *reference
                    {
                        return Err(invalid(format!(
                            "node {node} re-registered with other inputs, box or spec values"
                        )));
                    }
                } else {
                    for i in inputs {
                        if !self.inputs.contains_key(i) {
                            return Err(invalid(format!("node {node}: unknown input {i}")));
                        }
                    }
                    if spec.start_target == Some(crate::ledger::StartTarget::Boxed)
                        && spec_box.is_none()
                    {
                        return Err(invalid(format!(
                            "node {node}: start_target boxed needs a spec box (the campaign never chooses one)"
                        )));
                    }
                    if let Some(b) = spec_box {
                        match self.inputs.get(b) {
                            Some(i) if i.role == "box" => {}
                            Some(i) => {
                                return Err(invalid(format!(
                                    "node {node}: spec box {b} has role {:?}, not \"box\"",
                                    i.role
                                )));
                            }
                            None => {
                                return Err(invalid(format!(
                                    "node {node}: unknown spec box input {b}"
                                )));
                            }
                        }
                    }
                    self.nodes.insert(
                        node.clone(),
                        NodeRec {
                            state: NodeState::new(
                                spec_box.clone(),
                                terminal_bound.unwrap_or(self.terminal_bound),
                                NodeSpec::resolve(spec),
                            ),
                            inputs: inputs.clone(),
                            items_by_phase: [0; N_ITEM_PHASES],
                            accepted_by_level: [0; 3],
                            rule_set: RuleSetAcc::default(),
                            last_keyset: None,
                            stale_reason: None,
                            cpu_ms: 0,
                            paused: false,
                            reference: reference.clone(),
                        },
                    );
                }
            }
            Event::Node { node, ev } => self.apply_node_event(rec, node, ev)?,
            Event::ItemCreated {
                item,
                node,
                kind,
                rung,
                parent,
                inputs,
                label,
                weight,
            } => {
                if let Some(it) = self.items.get(item) {
                    if it.node != *node || it.rung != *rung || it.kind != *kind {
                        return Err(invalid(format!(
                            "item {item} re-created with other identity"
                        )));
                    }
                } else {
                    let n = self
                        .nodes
                        .get(node)
                        .ok_or_else(|| invalid(format!("item {item}: unknown node {node}")))?;
                    if n.state.phase == NodePhase::Stale {
                        return Err(invalid(format!("item {item}: node {node} is stale")));
                    }
                    if let Some(p) = parent {
                        match self.items.get(p) {
                            None => {
                                return Err(invalid(format!("item {item}: unknown parent {p}")));
                            }
                            Some(pi) if pi.phase == ItemPhase::Stale => {
                                return Err(invalid(format!("item {item}: parent {p} is stale")));
                            }
                            Some(_) => {}
                        }
                    }
                    for i in inputs {
                        if !self.inputs.contains_key(i) {
                            return Err(invalid(format!("item {item}: unknown input {i}")));
                        }
                    }
                    self.nodes.get_mut(node).unwrap().items_by_phase[ItemPhase::Open.index()] += 1;
                    if let Some(p) = parent {
                        self.items.get_mut(p).unwrap().children.push(item.clone());
                    }
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
                            generation: 0,
                            jobs: Vec::new(),
                            children: Vec::new(),
                            produced: None,
                            accepted: None,
                            economy: None,
                            retired: None,
                            exclusions: Vec::new(),
                            stale_from: None,
                            stale_input: None,
                            attention: None,
                            terminal_keys: 0,
                            terminal_preferred: false,
                            terminal_counted: false,
                            terminal_keyset: None,
                            ladder_exhausted: None,
                            paused: false,
                            failed_attempts: 0,
                            operator_retries: 0,
                            cpu_ms: 0,
                            created_seq: seq,
                            updated_seq: seq,
                        },
                    );
                    self.recheck_node(rec, node, &format!("item {item} created"), true);
                }
            }
            Event::Item { item, ev } => self.apply_item_event(rec, item, ev)?,
            Event::RuleEconomy { item, economy } => {
                let it = self
                    .items
                    .get(item)
                    .ok_or_else(|| invalid(format!("economy: unknown item {item}")))?;
                if !matches!(it.phase, ItemPhase::Accepted(_)) {
                    return Err(invalid(format!(
                        "economy of {item}: item is {}, not accepted",
                        it.phase.tag()
                    )));
                }
                self.items.get_mut(item).unwrap().economy = Some(Box::new(economy.clone()));
            }
            Event::JobQueued {
                job,
                item,
                rung,
                kind,
                spec_blake3,
                binary,
                reserved_bytes,
                inputs,
            } => {
                if let Some(j) = self.jobs.get(job) {
                    if j.item != *item || j.spec_blake3 != *spec_blake3 {
                        return Err(invalid(format!("job {job} re-queued with other identity")));
                    }
                } else {
                    let it = self
                        .items
                        .get(item)
                        .ok_or_else(|| invalid(format!("job {job}: unknown item {item}")))?;
                    if !matches!(it.phase, ItemPhase::Open | ItemPhase::InProgress) {
                        return Err(invalid(format!(
                            "job {job}: item {item} is {}",
                            it.phase.tag()
                        )));
                    }
                    if !self.inputs.contains_key(binary) {
                        return Err(invalid(format!(
                            "job {job}: binary {binary} not registered (M1)"
                        )));
                    }
                    if let Some(u) = self.unconfirmed_job_of(item) {
                        return Err(invalid(format!(
                            "job {job}: job {u} of item {item} has an unconfirmed launch (execution unknown): no new job for the same work until its run directory is authenticated or an operator retries the item"
                        )));
                    }
                    let n = &self.nodes[&it.node];
                    let mut ids: BTreeSet<&String> = BTreeSet::new();
                    ids.insert(binary);
                    ids.extend(inputs.iter());
                    ids.extend(it.inputs.iter());
                    ids.extend(n.inputs.iter());
                    ids.extend(n.state.spec_box.iter());
                    let ids: Vec<String> = ids.into_iter().cloned().collect();
                    let stamp = self
                        .stamp_digest(&ids)
                        .map_err(|i| invalid(format!("job {job}: unknown input {i}")))?;
                    let generation = it.generation;
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
                            generation,
                            inputs: ids,
                            inputs_digest: stamp,
                            stale: None,
                            attempt: 1,
                            state: AttemptState::Queued,
                            accept: None,
                            run_dir: None,
                            pid: None,
                            start_ticks: None,
                            boot_id: None,
                            sealed_table: None,
                            outcome: None,
                            attempt_spec: None,
                            prev_attempt_spec: None,
                            time_decided: false,
                            launch_session: None,
                            failures: 0,
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
                    self.pending_ops.insert(
                        op.clone(),
                        PendingOp {
                            action: action.clone(),
                            target: target.clone(),
                            seq,
                        },
                    );
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
            Event::Control { id, ev } => self.apply_control(rec, id, ev)?,
            // lane int5 (gate3 M2): informational, no state
            Event::SignalSent { .. } => {}
            Event::OrphanKilled { pid, job, .. } => {
                self.orphans_killed += 1;
                self.push_recent(
                    rec,
                    "orphan_killed",
                    format!(
                        "orphan pid {pid} killed (job {})",
                        job.as_deref().unwrap_or("?")
                    ),
                );
            }
            Event::VerifyCheckpoint { seq: at, clean, .. } => {
                if *at >= seq {
                    return Err(invalid(format!(
                        "verify checkpoint names seq {at}, not before this record"
                    )));
                }
                self.last_verify = Some((*at, *clean));
            }
            Event::ArtefactLost { path, items, .. } => {
                for i in items {
                    if !self.items.contains_key(i) {
                        return Err(invalid(format!("artefact_lost: unknown item {i}")));
                    }
                }
                let mut hit: Vec<String> = items.clone();
                for (id, it) in &self.items {
                    if it.accepted.as_ref().is_some_and(|a| a.record.path == *path)
                        && !hit.contains(id)
                    {
                        hit.push(id.clone());
                    }
                }
                let cause = format!("artefact:{path}");
                let n = self.stale_items(rec, hit, &cause);
                self.push_recent(
                    rec,
                    "artefact_lost",
                    format!("artefact {path} lost: {n} items marked stale"),
                );
            }
            Event::Export { rev, ev } => {
                let w = match ev {
                    ExportEvent::Candidate { .. } => "candidate",
                    ExportEvent::Published { .. } => "published",
                    ExportEvent::Revoked { .. } => "revoked",
                };
                self.exports.insert(rev.clone(), w.into());
            }
            Event::Note { .. } => self.progress_records += 1,
        }
        self.last_seq = seq;
        self.last_utc = rec.utc.clone();
        Ok(())
    }

    fn apply_provenance(
        &mut self,
        rec: &Record,
        id: &str,
        p: ProvenanceRec,
    ) -> Result<(), ReduceError> {
        let seq = rec.seq;
        if let Some(e) = self.provenance.get(id) {
            if e.files != p.files
                || e.producer != p.producer
                || e.description != p.description
                || e.source_campaign != p.source_campaign
                || e.level_claimed != p.level_claimed
                || e.kind != p.kind
                || e.manifest != p.manifest
                || e.producer_known != p.producer_known
                || e.provenance_class != p.provenance_class
            {
                return Err(ReduceError::Invalid {
                    seq,
                    why: format!("provenance {id} re-imported with different content"),
                });
            }
            return Ok(());
        }
        let text = format!(
            "{} {id} by {} ({} files, level claimed {})",
            p.kind,
            p.producer,
            p.files.len(),
            p.level_claimed.word()
        );
        self.provenance.insert(id.to_string(), p);
        self.push_recent(rec, "provenance", text);
        Ok(())
    }

    fn apply_input(
        &mut self,
        rec: &Record,
        input: &str,
        role: &str,
        path: &str,
        bytes: u64,
        blake3: &str,
    ) {
        let seq = rec.seq;
        match self.inputs.get_mut(input) {
            Some(cur) if cur.blake3 == blake3 => {} // repeat (or re-verification at resume): no-op
            Some(cur) => {
                let old = std::mem::replace(&mut cur.blake3, blake3.to_string());
                cur.bytes = bytes;
                cur.path = path.to_string();
                cur.versions += 1;
                cur.registered_seq = seq;
                let n = self.mark_stale_by_input(rec, input);
                self.push_recent(
                    rec,
                    "stale_inputs",
                    format!(
                        "input {input} changed ({}.. -> {}..): {n} items marked stale",
                        &old[..old.len().min(8)],
                        &blake3[..blake3.len().min(8)]
                    ),
                );
            }
            None => {
                self.inputs.insert(
                    input.to_string(),
                    InputRec {
                        role: role.to_string(),
                        path: path.to_string(),
                        bytes,
                        blake3: blake3.to_string(),
                        versions: 1,
                        registered_seq: seq,
                    },
                );
            }
        }
    }

    /// A changed input (audit A C1): every job stamped with it becomes stale (it can feed no result); every
    /// item listing it (or on a node listing it), and every item whose current result or work depends on a stale
    /// job, becomes STALE (never deleted), transitively through child items; nodes listing it become stale,
    /// other affected nodes lose a closed verdict automatically.
    fn mark_stale_by_input(&mut self, rec: &Record, input: &str) -> usize {
        let mut stale_jobs = BTreeSet::new();
        let mut newly = Vec::new();
        for (id, j) in self.jobs.iter() {
            if j.inputs.iter().any(|i| i == input) && j.stale.is_none() {
                newly.push(id.clone());
                stale_jobs.insert(id.clone());
            } else if j.stale.as_deref() == Some(input) {
                stale_jobs.insert(id.clone());
            }
        }
        for id in newly {
            self.jobs.get_mut(&id).unwrap().stale = Some(input.to_string());
        }
        let nodes_hit: BTreeSet<String> = self
            .nodes
            .iter()
            .filter(|(_, n)| {
                n.inputs.iter().any(|i| i == input) || n.state.spec_box.as_deref() == Some(input)
            })
            .map(|(k, _)| k.clone())
            .collect();
        let mut hit = Vec::new();
        for (id, it) in &self.items {
            if it.phase == ItemPhase::Stale {
                continue;
            }
            let holds_result = matches!(
                it.phase,
                ItemPhase::InProgress
                    | ItemPhase::Produced
                    | ItemPhase::Accepted(_)
                    | ItemPhase::Retired
                    | ItemPhase::TerminalCandidate
                    | ItemPhase::Terminal
                    | ItemPhase::Attention(_)
            );
            let via_job = holds_result
                && (it
                    .jobs
                    .iter()
                    .any(|j| stale_jobs.contains(j) && self.jobs[j].generation == it.generation)
                    || it
                        .produced
                        .as_ref()
                        .is_some_and(|p| stale_jobs.contains(&p.job))
                    || matches!(it.accepted.as_deref(), Some(AcceptedRec { source: AcceptedFrom::Job { job, .. }, .. }) if stale_jobs.contains(job)));
            if nodes_hit.contains(&it.node) || it.inputs.iter().any(|i| i == input) || via_job {
                hit.push(id.clone());
            }
        }
        let n = self.stale_items(rec, hit, input);
        for node in nodes_hit {
            let nr = self.nodes.get_mut(&node).unwrap();
            nr.state.phase = NodePhase::Stale;
            nr.stale_reason = Some(format!("input {input} changed"));
        }
        n
    }

    /// Marks `items` and (transitively) their children stale with cause `input`; returns the number marked.
    fn stale_items(&mut self, rec: &Record, items: Vec<String>, input: &str) -> usize {
        let seq = rec.seq;
        let mut todo = items;
        let mut done: BTreeSet<String> = BTreeSet::new();
        while let Some(id) = todo.pop() {
            if !done.insert(id.clone()) {
                continue;
            }
            // the parent -> children index: O(children) per item (audit-ledger2 M4: was a scan of every item)
            for cid in &self.items[&id].children {
                if self.items[cid].phase != ItemPhase::Stale {
                    todo.push(cid.clone());
                }
            }
        }
        let mut nodes = BTreeSet::new();
        let mut n = 0;
        for id in &done {
            let it = &self.items[id];
            if it.phase == ItemPhase::Stale {
                continue;
            }
            let old_elem = if matches!(it.phase, ItemPhase::Accepted(_)) {
                elem_of(id, it)
            } else {
                None
            };
            let it = self.items.get_mut(id).unwrap();
            let from = std::mem::replace(&mut it.phase, ItemPhase::Stale);
            let nr = self.nodes.get_mut(&it.node).unwrap();
            nr.items_by_phase[from.index()] -= 1;
            nr.items_by_phase[ItemPhase::Stale.index()] += 1;
            if let ItemPhase::Accepted(l) = from {
                nr.accepted_by_level[l as usize] -= 1;
            }
            if let Some(e) = old_elem {
                nr.rule_set.sub(e);
            }
            if from == ItemPhase::Terminal {
                Self::uncount_terminal(nr, &mut self.terminals_total, it);
            }
            it.stale_from = Some(from.tag().to_string());
            it.stale_input = Some(input.to_string());
            it.updated_seq = seq;
            nodes.insert(it.node.clone());
            n += 1;
        }
        for node in nodes {
            self.recheck_node(rec, &node, &format!("items stale ({input})"), false);
        }
        n
    }

    fn uncount_terminal(nr: &mut NodeRec, total: &mut u64, it: &mut ItemRec) {
        if it.terminal_counted {
            nr.state.terminals = nr.state.terminals.saturating_sub(it.terminal_keys);
        }
        if it.terminal_preferred {
            nr.state.preferred_terminals = nr
                .state
                .preferred_terminals
                .saturating_sub(it.terminal_keys);
        }
        *total = total.saturating_sub(it.terminal_keys as u64);
        it.terminal_keys = 0;
        it.terminal_counted = false;
    }

    /// The derived node verdict (audit A C2): a closed / certified node whose binding no longer holds (rule set,
    /// dispatch order, an open item, a rule below the certification level) goes back to running; an OPEN node
    /// with new active work (`activity`) goes back to running.
    fn recheck_node(&mut self, rec: &Record, node: &str, cause: &str, activity: bool) {
        let Some(n) = self.nodes.get(node) else {
            return;
        };
        let reopen = match n.state.phase {
            NodePhase::Closed | NodePhase::Certified => {
                let ctx = n.context();
                let binding_ok = n.state.closure.as_ref().is_some_and(|c| {
                    c.defect_keys == 0
                        && c.rule_set_digest == ctx.rule_set_digest
                        && n.state.dispatch_order.as_deref()
                            == Some(c.dispatch_order_digest.as_str())
                });
                let cert_ok =
                    n.state.phase == NodePhase::Closed || ctx.below(n.state.required_cert()) == 0;
                !(binding_ok && ctx.items_open == 0 && cert_ok)
            }
            NodePhase::Open => activity,
            _ => false,
        };
        if !reopen {
            return;
        }
        let n = self.nodes.get_mut(node).unwrap();
        let was = n.state.phase;
        n.state.phase = NodePhase::Running;
        n.state.reopens += 1;
        n.state.reopen_cause = Some(cause.to_string());
        self.push_recent(
            rec,
            "node_reopened",
            format!("{node}: {} -> running ({cause})", was.tag()),
        );
    }

    fn apply_node_event(
        &mut self,
        rec: &Record,
        node: &str,
        ev: &NodeEvent,
    ) -> Result<(), ReduceError> {
        let seq = rec.seq;
        let n = self.nodes.get(node).ok_or_else(|| ReduceError::Invalid {
            seq,
            why: format!("unknown node {node}"),
        })?;
        let next = apply_node(&n.state, ev, &n.context())
            .map_err(|err| ReduceError::Illegal { seq, err })?;
        let changed = next != n.state;
        let prev_target = n.state.target.clone();
        let n = self.nodes.get_mut(node).unwrap();
        n.state = next;
        match ev {
            NodeEvent::ClosureResult { keyset, .. } if changed => n.last_keyset = keyset.clone(),
            NodeEvent::MarkStale { reason } => n.stale_reason = Some(reason.clone()),
            NodeEvent::Reopen { .. } => n.stale_reason = None,
            _ => {}
        }
        // An explicit node mark_stale marks its items stale too (audit A m11).
        if let NodeEvent::MarkStale { reason } = ev {
            let ids: Vec<String> = self
                .items
                .iter()
                .filter(|(_, it)| it.node == node && it.phase != ItemPhase::Stale)
                .map(|(k, _)| k.clone())
                .collect();
            self.stale_items(rec, ids, &format!("node:{reason}"));
        }
        // Reopen to FULL from a box: box-only retirements are reinstated (coverage restored, audit A M5).
        if matches!(ev, NodeEvent::Reopen { .. })
            && matches!(prev_target, ClosureTarget::Boxed { .. })
        {
            let ids: Vec<String> = self
                .items
                .iter()
                .filter(|(_, it)| {
                    it.node == node
                        && it.phase == ItemPhase::Retired
                        && it.retired.as_ref().is_some_and(|r| r.proof.box_only())
                })
                .map(|(k, _)| k.clone())
                .collect();
            for id in ids {
                self.reinstate(rec, &id);
            }
        }
        if changed {
            let text = match ev {
                NodeEvent::ClosureResult {
                    round,
                    defect_keys,
                    closure_keys,
                    envelope,
                    ..
                } => format!(
                    "{node}: closure round {round}, defect {defect_keys} of {closure_keys} keys ({envelope:?} envelope)"
                ),
                NodeEvent::AttemptEnded {
                    blockers, reason, ..
                } => format!(
                    "{node}: attempt ended ({reason}): {} faces, {} loci, {} keys block closure",
                    blockers.faces, blockers.loci, blockers.keys
                ),
                NodeEvent::FallBackToBox { box_id } => {
                    format!("{node}: full closure not reached; falling back to spec box {box_id}")
                }
                NodeEvent::FixerRoundBegin { round, .. } => {
                    format!("{node}: fixer round {round} begins")
                }
                NodeEvent::Assembled { round, .. } => {
                    format!("{node}: table set assembled (round {round})")
                }
                _ => {
                    let s = &self.nodes[node].state;
                    format!("{node}: {} ({})", s.phase.tag(), s.closure_kind())
                }
            };
            self.push_recent(rec, "node", text);
            if matches!(ev, NodeEvent::Assembled { .. }) {
                self.recheck_node(rec, node, "dispatch order changed", false);
            }
        }
        Ok(())
    }

    /// Retired -> accepted at its old level (no checks; callers check).
    fn reinstate(&mut self, rec: &Record, id: &str) {
        let it = self.items.get_mut(id).unwrap();
        let Some(level) = it.accepted.as_ref().map(|a| a.level) else {
            return;
        };
        it.phase = ItemPhase::Accepted(level);
        it.retired = None;
        it.updated_seq = rec.seq;
        let e = elem_of(id, it).unwrap();
        let nr = self.nodes.get_mut(&it.node).unwrap();
        nr.items_by_phase[ItemPhase::Retired.index()] -= 1;
        nr.items_by_phase[ItemPhase::Accepted(level).index()] += 1;
        nr.accepted_by_level[level as usize] += 1;
        nr.rule_set.add(e);
        let node = it.node.clone();
        self.push_recent(
            rec,
            "rule_reinstated",
            format!("{node} {id}: rule reinstated"),
        );
    }

    /// Checks a job may feed a result of `item` (audit A C1, C3).
    fn job_feeds(&self, item: &str, job: &str, attempt: u32) -> Result<&JobRec, String> {
        let it = &self.items[item];
        let j = self.jobs.get(job).ok_or(format!("unknown job {job}"))?;
        if j.item != item {
            return Err(format!("job {job} belongs to item {}", j.item));
        }
        if j.attempt != attempt {
            return Err(format!(
                "job {job}: attempt {attempt} is not the current attempt {}",
                j.attempt
            ));
        }
        if j.generation != it.generation {
            return Err(format!(
                "job {job} was queued for generation {} of the item, which is now at generation {} (reopened since)",
                j.generation, it.generation
            ));
        }
        if let Some(i) = &j.stale {
            return Err(format!(
                "job {job} is stale: input {i} changed after it was queued"
            ));
        }
        if self.stamp_digest(&j.inputs).as_deref() != Ok(j.inputs_digest.as_str()) {
            return Err(format!("job {job}: its inputs differ from its stamp"));
        }
        if !j.state.is_settled() || matches!(j.state, AttemptState::Queued) {
            return Err(format!("job {job} attempt {attempt} has not finished"));
        }
        match j.accept {
            Some(a) if accept_permits_records(a) => {}
            Some(a) => {
                return Err(format!(
                    "the contract row of job {job} attempt {attempt} accepts {}",
                    accept_word(a)
                ));
            }
            None => return Err(format!("job {job} attempt {attempt} has no finish record")),
        }
        if j.sealed_table.is_none() {
            return Err(format!("job {job} attempt {attempt} has no sealed table"));
        }
        Ok(j)
    }

    fn apply_item_event(
        &mut self,
        rec: &Record,
        item: &str,
        ev: &crate::ledger::ItemEvent,
    ) -> Result<(), ReduceError> {
        use crate::ledger::ItemEvent as IE;
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let illegal = |err: IllegalTransition| ReduceError::Illegal { seq, err };
        let it = self
            .items
            .get(item)
            .ok_or_else(|| invalid(format!("unknown item {item}")))?;
        let node_id = it.node.clone();
        // Events that keep the phase (no machine transition).
        match ev {
            IE::ExcludeKeys { keys, .. } => {
                if !matches!(it.phase, ItemPhase::Accepted(_)) {
                    return Err(invalid(format!(
                        "exclude_keys of {item}: item is {}, not accepted",
                        it.phase.tag()
                    )));
                }
                if it.exclusions.contains(&keys.blake3) {
                    return Ok(());
                }
                let old = elem_of(item, it).unwrap();
                let it = self.items.get_mut(item).unwrap();
                it.exclusions.push(keys.blake3.clone());
                it.updated_seq = seq;
                let new = elem_of(item, it).unwrap();
                let nr = self.nodes.get_mut(&node_id).unwrap();
                nr.rule_set.sub(old);
                nr.rule_set.add(new);
                self.push_recent(
                    rec,
                    "key_excluded",
                    format!("{node_id} {item}: keys excluded"),
                );
                self.recheck_node(rec, &node_id, &format!("keys excluded from {item}"), false);
                return Ok(());
            }
            IE::Replace {
                replaces,
                containment,
            } => return self.apply_replace(rec, item, replaces, containment),
            _ => {}
        }
        let machine_ev = match ev {
            IE::Start { .. } => ItemMachineEvent::Start,
            IE::Produce { .. } => ItemMachineEvent::Produce,
            IE::Accept { level, .. } => ItemMachineEvent::Accept(*level),
            IE::Import { level, .. } => ItemMachineEvent::Import(*level),
            IE::Upgrade { level, .. } => ItemMachineEvent::Upgrade(*level),
            IE::Reject { reason, .. } => ItemMachineEvent::Reject(reason.clone()),
            IE::Escalate { to, .. } => ItemMachineEvent::Escalate(*to),
            IE::Requeue { .. } => ItemMachineEvent::Requeue,
            IE::ProposeTerminal { .. } => ItemMachineEvent::ProposeTerminal,
            IE::DeclareTerminal { .. } => ItemMachineEvent::DeclareTerminal,
            IE::MarkStale { .. } => ItemMachineEvent::MarkStale,
            IE::Reopen { .. } => ItemMachineEvent::Reopen,
            IE::Retire { .. } => ItemMachineEvent::Retire,
            IE::Reinstate { .. } => ItemMachineEvent::Reinstate(
                it.accepted
                    .as_ref()
                    .map(|a| a.level)
                    .unwrap_or(EvidenceLevel::GeometryAccepted),
            ),
            IE::Attention { code, .. } => ItemMachineEvent::Attention(code.clone()),
            IE::ExcludeKeys { .. } | IE::Replace { .. } => unreachable!(),
        };
        let cur = ItemState {
            phase: it.phase.clone(),
            rung: it.rung,
        };
        let next = apply_item(&cur, &machine_ev).map_err(illegal)?;
        let repeat = next == cur;
        let node = &self.nodes[&node_id];
        // Additional guards (all before mutation).
        let mut accepted = None;
        let mut produced = None;
        match ev {
            IE::Produce {
                job,
                attempt,
                table,
            } => {
                if repeat {
                    let p = it.produced.as_ref();
                    if p.is_none_or(|p| p.job != *job || p.attempt != *attempt || p.table != *table)
                    {
                        return Err(invalid(format!(
                            "produce of {item}: already produced from another attempt"
                        )));
                    }
                } else {
                    let j = self
                        .job_feeds(item, job, *attempt)
                        .map_err(|w| invalid(format!("produce of {item}: {w}")))?;
                    if j.sealed_table.as_ref().map(|t| &t.blake3) != Some(&table.blake3) {
                        return Err(invalid(format!(
                            "produce of {item}: table {} is not the sealed table of job {job}",
                            short(&table.blake3)
                        )));
                    }
                    produced = Some(Box::new(ProducedRec {
                        job: job.clone(),
                        attempt: *attempt,
                        table: table.clone(),
                        seq,
                    }));
                }
            }
            IE::Accept {
                rule,
                level,
                record,
                source,
                elided,
                evidence,
                round,
            } => {
                if *elided {
                    return Err(invalid(format!(
                        "accept of {rule}: records with elided witness loci are unusable (fail closed)"
                    )));
                }
                check_level_evidence(*level, evidence.as_ref())
                    .map_err(|w| invalid(format!("accept of {rule}: {w}")))?;
                if repeat {
                    let a = it.accepted.as_ref().unwrap();
                    if a.rule != *rule || a.record != *record {
                        return Err(invalid(format!(
                            "item {item} already accepted with rule {}",
                            a.rule
                        )));
                    }
                    return Ok(());
                }
                let p = it.produced.as_ref().ok_or_else(|| {
                    invalid(format!(
                        "accept of {rule}: item {item} has no produced result"
                    ))
                })?;
                self.job_feeds(item, &p.job, p.attempt)
                    .map_err(|w| invalid(format!("accept of {rule}: {w}")))?;
                let from = match source {
                    AcceptSource::Job {
                        job,
                        attempt,
                        table_blake3,
                    } => {
                        if *job != p.job || *attempt != p.attempt || *table_blake3 != p.table.blake3
                        {
                            return Err(invalid(format!(
                                "accept of {rule}: job {job} attempt {attempt} is not the attempt that produced this item's result ({} attempt {}); accepted records come only from that sealed table",
                                p.job, p.attempt
                            )));
                        }
                        AcceptedFrom::Job {
                            job: job.clone(),
                            attempt: *attempt,
                            table_blake3: table_blake3.clone(),
                        }
                    }
                    AcceptSource::Evidence { evidence: e } => {
                        let pr = self.provenance.get(e).ok_or_else(|| {
                            invalid(format!("accept of {rule}: unknown evidence {e}"))
                        })?;
                        if let Some((i, at)) = self.input_changed_after(pr.seq, it) {
                            return Err(invalid(format!(
                                "accept of {rule}: evidence {e} (recorded at seq {}) is older than the change of input {i} at seq {at}: evidence made under the old input never undoes a stale verdict (audit-ledger2 M2)",
                                pr.seq
                            )));
                        }
                        if !pr.producer_known {
                            return Err(invalid(format!(
                                "accept of {rule}: evidence {e} is provenance only (producer digest unknown: {}); it gives no evidence level by itself",
                                pr.provenance_class.as_deref().unwrap_or("-")
                            )));
                        }
                        if *level > pr.level_claimed {
                            return Err(invalid(format!(
                                "accept of {rule}: level {} above what evidence {e} claims ({})",
                                level.word(),
                                pr.level_claimed.word()
                            )));
                        }
                        AcceptedFrom::Evidence {
                            evidence: e.clone(),
                        }
                    }
                };
                accepted = Some(Box::new(AcceptedRec {
                    rule: rule.clone(),
                    level: *level,
                    record: record.clone(),
                    source: from,
                    round: *round,
                    seq,
                }));
            }
            IE::Import {
                rule,
                level,
                record,
                provenance,
            } => {
                let pr = self.provenance.get(provenance).ok_or_else(|| {
                    invalid(format!("import of {rule}: unknown provenance {provenance}"))
                })?;
                if !pr.producer_known {
                    return Err(invalid(format!(
                        "import of {rule}: {provenance} is provenance only (producer digest unknown: {}); it gives no evidence level by itself",
                        pr.provenance_class.as_deref().unwrap_or("-")
                    )));
                }
                if *level > pr.level_claimed {
                    return Err(invalid(format!(
                        "import of {rule}: level {} above what {provenance} claims ({})",
                        level.word(),
                        pr.level_claimed.word()
                    )));
                }
                if !repeat && let Some((i, at)) = self.input_changed_after(pr.seq, it) {
                    return Err(invalid(format!(
                        "import of {rule}: {provenance} (recorded at seq {}) is older than the change of input {i} at seq {at}: a rule made under the old input never undoes a stale verdict (audit-ledger2 M2)",
                        pr.seq
                    )));
                }
                if repeat {
                    let a = it.accepted.as_ref().unwrap();
                    if a.rule != *rule || a.record != *record {
                        return Err(invalid(format!(
                            "item {item} already accepted with rule {}",
                            a.rule
                        )));
                    }
                    return Ok(());
                }
                accepted = Some(Box::new(AcceptedRec {
                    rule: rule.clone(),
                    level: *level,
                    record: record.clone(),
                    source: AcceptedFrom::Import {
                        provenance: provenance.clone(),
                    },
                    round: None,
                    seq,
                }));
            }
            IE::Upgrade { level, evidence } if !repeat => {
                check_level_evidence(*level, Some(evidence))
                    .map_err(|w| invalid(format!("upgrade of {item}: {w}")))?;
            }
            IE::ProposeTerminal {
                preferred,
                exhausted,
                ..
            } if !repeat => {
                if let Some(u) = self.unconfirmed_job_of(item) {
                    return Err(invalid(format!(
                        "propose_terminal of {item}: job {u} has an unconfirmed launch (execution unknown): its run directory must be authenticated first"
                    )));
                }
                if !*preferred {
                    if exhausted.is_none() {
                        return Err(invalid(format!(
                            "terminal proposal of {item}: a non-preferred terminal cites the record of its exhausted ladder"
                        )));
                    }
                    if it.rung < node.state.spec.terminal_rung {
                        return Err(invalid(format!(
                            "terminal proposal of {item}: rung {} is below the spec's terminal rung {}",
                            it.rung, node.state.spec.terminal_rung
                        )));
                    }
                }
            }
            IE::DeclareTerminal {
                count,
                keys,
                preferred,
                ..
            } => {
                if repeat {
                    if it.terminal_keys != *count
                        || it.terminal_keyset.as_deref() != Some(keys)
                        || it.terminal_preferred != *preferred
                    {
                        return Err(invalid(format!(
                            "terminal of {item} already declared with other keys"
                        )));
                    }
                    return Ok(());
                }
                if *count == 0 {
                    return Err(invalid(format!("terminal of {item}: count must be >= 1")));
                }
                if *preferred != it.terminal_preferred {
                    return Err(invalid(format!(
                        "terminal of {item}: preferred flag differs from the proposal"
                    )));
                }
                let counted = !*preferred || node.state.spec.preferred_count_against_bound;
                if counted {
                    apply_node_internal(
                        &node.state,
                        NodeInternal::TerminalDeclared { count: *count },
                    )
                    .map_err(illegal)?;
                }
            }
            IE::Retire {
                rule,
                proof,
                superseded_by,
            } if !repeat => {
                let a = it
                    .accepted
                    .as_ref()
                    .ok_or_else(|| invalid(format!("retire: {item} has no accepted rule")))?;
                if a.rule != *rule {
                    return Err(invalid(format!(
                        "retire: {item} carries rule {}, not {rule}",
                        a.rule
                    )));
                }
                if matches!(proof, RetirementProof::ReplacedBy { .. }) {
                    if superseded_by.is_none() {
                        return Err(invalid(
                            "retire by replacement must name the superseding item".into(),
                        ));
                    }
                    if self.mode == CampaignMode::Optimise {
                        return Err(invalid(
                            "optimise mode: replacements go through item.replace (economy + containment)"
                                .into(),
                        ));
                    }
                }
                if let Some(by) = superseded_by {
                    let b = self
                        .items
                        .get(by)
                        .ok_or_else(|| invalid(format!("retire: unknown superseding item {by}")))?;
                    if by == item || b.node != it.node || !matches!(b.phase, ItemPhase::Accepted(_))
                    {
                        return Err(invalid(format!(
                            "retire: {by} is not another accepted item of node {}",
                            it.node
                        )));
                    }
                }
                apply_node_internal(
                    &node.state,
                    NodeInternal::Retirement {
                        box_only: proof.box_only(),
                    },
                )
                .map_err(illegal)?;
            }
            IE::Reinstate { .. } if !repeat => {
                if it.accepted.is_none() {
                    return Err(invalid(format!("reinstate: {item} has no accepted record")));
                }
                if node.state.phase == NodePhase::Stale {
                    return Err(invalid(format!("reinstate of {item}: node is stale")));
                }
            }
            IE::Reopen { .. } if !repeat && node.state.phase == NodePhase::Stale => {
                return Err(invalid(format!(
                    "reopen of {item}: node {node_id} is stale (reopen the node first)"
                )));
            }
            IE::Reopen { .. } if !repeat => {
                // audit A m11: an item whose child carries its work (escalation) is not revived
                if let Some(c) = self.live_child_of(item) {
                    return Err(invalid(format!(
                        "reopen of {item}: its child {c} carries the work"
                    )));
                }
                // an unconfirmed launch is never retried automatically (operator retry only)
                if let Some(u) = self.unconfirmed_job_of(item) {
                    return Err(invalid(format!(
                        "reopen of {item}: job {u} has an unconfirmed launch (execution unknown); only an operator retry reopens it"
                    )));
                }
            }
            IE::Requeue { .. } | IE::Escalate { .. } if !repeat => {
                if let Some(u) = self.unconfirmed_job_of(item) {
                    return Err(invalid(format!(
                        "{} of {item}: job {u} has an unconfirmed launch (execution unknown): its run directory must be authenticated first (or an operator retries the item)",
                        machine_ev.tag()
                    )));
                }
            }
            _ => {}
        }
        if repeat {
            return Ok(());
        }
        // Mutation.
        let old_elem = if matches!(cur.phase, ItemPhase::Accepted(_)) {
            elem_of(item, it)
        } else {
            None
        };
        let it = self.items.get_mut(item).unwrap();
        let from = std::mem::replace(&mut it.phase, next.phase.clone());
        it.updated_seq = seq;
        let n = self.nodes.get_mut(&node_id).unwrap();
        n.items_by_phase[from.index()] -= 1;
        n.items_by_phase[next.phase.index()] += 1;
        if let ItemPhase::Accepted(l) = from {
            n.accepted_by_level[l as usize] -= 1;
        }
        if let ItemPhase::Accepted(l) = next.phase {
            n.accepted_by_level[l as usize] += 1;
        }
        if let Some(e) = old_elem {
            n.rule_set.sub(e);
        }
        if n.state.phase == NodePhase::Registered && matches!(ev, IE::Start { .. }) {
            n.state.phase = NodePhase::Running;
        }
        if next.phase == ItemPhase::Open && from != ItemPhase::Open {
            it.generation += 1;
            it.produced = None;
            it.attention = None;
        }
        if !matches!(next.phase, ItemPhase::Attention(_)) {
            it.attention = None;
        }
        let label = it.label.clone();
        let text = match ev {
            IE::Produce { .. } => {
                it.produced = produced;
                None
            }
            IE::Accept { rule, level, .. } => {
                self.rules.accepted += 1;
                it.accepted = accepted;
                Some(format!(
                    "{node_id} {label}: rule {rule} accepted ({})",
                    level.word()
                ))
            }
            IE::Import {
                rule,
                level,
                provenance,
                ..
            } => {
                self.rules.imported += 1;
                it.accepted = accepted;
                Some(format!(
                    "{node_id} {label}: rule {rule} imported from {provenance} ({})",
                    level.word()
                ))
            }
            IE::Retire {
                rule,
                proof,
                superseded_by,
            } => {
                self.rules.retired += 1;
                it.retired = Some(Box::new(RetiredRec {
                    proof: proof.clone(),
                    superseded_by: superseded_by.clone(),
                    seq,
                }));
                Some(match superseded_by {
                    Some(by) => format!(
                        "{node_id} {label}: rule {rule} superseded by {by} ({})",
                        proof.word()
                    ),
                    None => format!("{node_id} {label}: rule {rule} retired ({})", proof.word()),
                })
            }
            IE::Reinstate { reason } => {
                it.retired = None;
                Some(format!("{node_id} {label}: rule reinstated ({reason})"))
            }
            IE::Upgrade { level, .. } => {
                self.rules.upgrades += 1;
                if let Some(a) = it.accepted.as_mut() {
                    a.level = *level;
                }
                Some(format!("{node_id} {label}: evidence -> {}", level.word()))
            }
            IE::Reject { reason, .. } => {
                self.rules.rejected += 1;
                *self
                    .rules
                    .rejected_by_reason
                    .entry(reason.clone())
                    .or_default() += 1;
                if matches!(from, ItemPhase::Accepted(_)) {
                    it.accepted = None;
                }
                Some(format!("{node_id} {label}: rejected ({reason})"))
            }
            IE::Escalate { to, reason } => {
                Some(format!("{node_id} {label}: escalated to {to} ({reason})"))
            }
            IE::DeclareTerminal {
                count,
                keys,
                preferred,
                ..
            } => {
                let counted = !*preferred || n.state.spec.preferred_count_against_bound;
                if counted {
                    n.state.terminals += *count;
                }
                if *preferred {
                    n.state.preferred_terminals += *count;
                }
                self.terminals_total += *count as u64;
                it.terminal_keys = *count;
                it.terminal_counted = counted;
                it.terminal_keyset = Some(Box::new(keys.clone()));
                Some(format!(
                    "{node_id} {label}: {count} terminal key(s) declared{}",
                    if *preferred { " (preferred)" } else { "" }
                ))
            }
            IE::ProposeTerminal {
                reason,
                preferred,
                exhausted,
            } => {
                it.terminal_preferred = *preferred;
                it.ladder_exhausted = exhausted.clone().map(Box::new);
                Some(format!("{node_id} {label}: terminal candidate ({reason})"))
            }
            IE::MarkStale { input } => {
                it.stale_from = Some(from.tag().to_string());
                it.stale_input = Some(input.clone());
                if from == ItemPhase::Terminal {
                    Self::uncount_terminal(n, &mut self.terminals_total, it);
                }
                Some(format!("{node_id} {label}: stale ({input})"))
            }
            IE::Reopen { .. } => {
                it.stale_from = None;
                it.stale_input = None;
                it.accepted = None;
                it.exclusions.clear();
                it.terminal_preferred = false;
                it.ladder_exhausted = None;
                None
            }
            IE::Attention { code, text } => {
                it.attention = Some(text.clone());
                Some(format!(
                    "{node_id} {label}: needs attention ({code}): {text}"
                ))
            }
            _ => None,
        };
        // New rule-set element if live now.
        let it = &self.items[item];
        if matches!(it.phase, ItemPhase::Accepted(_))
            && let Some(e) = elem_of(item, it)
        {
            self.nodes.get_mut(&node_id).unwrap().rule_set.add(e);
        }
        if let Some(t) = text {
            let kind = match ev {
                IE::Accept { .. } => "item_accepted",
                IE::Import { .. } => "item_imported",
                IE::Upgrade { .. } => "item_upgraded",
                IE::Reject { .. } => "item_rejected",
                IE::Escalate { .. } => "item_escalated",
                IE::DeclareTerminal { .. } => "terminal_declared",
                IE::ProposeTerminal { .. } => "terminal_proposed",
                IE::Retire { .. } => "rule_retired",
                IE::Reinstate { .. } => "rule_reinstated",
                IE::Attention { .. } => "attention",
                _ => "item_stale",
            };
            self.push_recent(rec, kind, t);
        }
        let activity = matches!(next.phase, ItemPhase::Open | ItemPhase::InProgress);
        self.recheck_node(
            rec,
            &node_id,
            &format!("item {item} {} -> {}", from.tag(), next.phase.tag()),
            activity,
        );
        Ok(())
    }

    /// U-13: `item` (accepted) replaces `replaces` (accepted, same node): Pareto-better economy and a
    /// containment proof; each replaced item becomes `retired` (replaced_by, superseded by `item`).
    fn apply_replace(
        &mut self,
        rec: &Record,
        item: &str,
        replaces: &[String],
        containment: &FileRef,
    ) -> Result<(), ReduceError> {
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let it = &self.items[item];
        if !matches!(it.phase, ItemPhase::Accepted(_)) {
            return Err(invalid(format!("replace: {item} is not accepted")));
        }
        if replaces.is_empty() {
            return Err(invalid("replace: nothing named".into()));
        }
        let all_done = replaces.iter().all(|r| {
            self.items.get(r).is_some_and(|x| {
                x.phase == ItemPhase::Retired
                    && x.retired
                        .as_ref()
                        .is_some_and(|rr| rr.superseded_by.as_deref() == Some(item))
            })
        });
        if all_done {
            return Ok(());
        }
        let new_e = it
            .economy
            .clone()
            .ok_or_else(|| invalid(format!("replace: {item} has no economy measure")))?;
        let mut old_e = RuleEconomy::default();
        let mut seen = BTreeSet::new();
        for r in replaces {
            if !seen.insert(r) || r == item {
                return Err(invalid(format!(
                    "replace: {r} named twice or is the replacer"
                )));
            }
            let x = self
                .items
                .get(r)
                .ok_or_else(|| invalid(format!("replace: unknown item {r}")))?;
            if x.node != it.node || !matches!(x.phase, ItemPhase::Accepted(_)) {
                return Err(invalid(format!(
                    "replace: {r} is not an accepted item of node {}",
                    it.node
                )));
            }
            let e = x
                .economy
                .as_ref()
                .ok_or_else(|| invalid(format!("replace: {r} has no economy measure")))?;
            old_e.add(e);
        }
        if !new_e.strictly_better_than(&old_e) {
            return Err(invalid(format!(
                "replace: the economy of {item} is not strictly better than that of the replaced rules (no measure worse, one smaller)"
            )));
        }
        let node_id = it.node.clone();
        let node = &self.nodes[&node_id];
        apply_node_internal(&node.state, NodeInternal::Retirement { box_only: false })
            .map_err(|err| ReduceError::Illegal { seq, err })?;
        for r in replaces {
            let x = &self.items[r];
            let e = elem_of(r, x).unwrap();
            let level = match x.phase {
                ItemPhase::Accepted(l) => l,
                _ => unreachable!(),
            };
            let x = self.items.get_mut(r).unwrap();
            x.phase = ItemPhase::Retired;
            x.updated_seq = seq;
            x.retired = Some(Box::new(RetiredRec {
                proof: RetirementProof::ReplacedBy {
                    proof: containment.clone(),
                },
                superseded_by: Some(item.to_string()),
                seq,
            }));
            let nr = self.nodes.get_mut(&node_id).unwrap();
            nr.items_by_phase[ItemPhase::Accepted(level).index()] -= 1;
            nr.items_by_phase[ItemPhase::Retired.index()] += 1;
            nr.accepted_by_level[level as usize] -= 1;
            nr.rule_set.sub(e);
            self.rules.retired += 1;
        }
        self.rules.replaced += 1;
        self.push_recent(
            rec,
            "rule_replaced",
            format!(
                "{node_id} {item}: replaces {} rules (economy better)",
                replaces.len()
            ),
        );
        self.recheck_node(rec, &node_id, &format!("{item} replaced rules"), false);
        Ok(())
    }

    fn apply_job_event(
        &mut self,
        rec: &Record,
        job: &str,
        attempt: u32,
        ev: &crate::ledger::JobEvent,
    ) -> Result<(), ReduceError> {
        use crate::ledger::JobEvent as JE;
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        let illegal = |err: IllegalTransition| ReduceError::Illegal { seq, err };
        let j = self
            .jobs
            .get(job)
            .ok_or_else(|| invalid(format!("unknown job {job}")))?;
        let machine_ev = match ev {
            JE::Launch { .. } => AttemptEvent::Launch,
            JE::Started { .. } => AttemptEvent::Started,
            JE::Finished {
                class,
                accept,
                sealed_output,
                ..
            } => AttemptEvent::Finished {
                class: *class,
                accept: *accept,
                sealed: sealed_output.is_some(),
            },
            JE::Interrupt { .. } => AttemptEvent::Interrupt,
            JE::Lose { .. } => AttemptEvent::Lose,
            JE::Requeue { .. } => AttemptEvent::Requeue,
            JE::Retry { .. } => AttemptEvent::Retry,
            JE::Cancel { .. } => AttemptEvent::Cancel,
            JE::Unconfirmed { .. } => AttemptEvent::Unconfirmed,
            JE::NotSpawned { .. } => AttemptEvent::NotSpawned,
        };
        let new_attempt = matches!(machine_ev, AttemptEvent::Requeue | AttemptEvent::Retry);
        // Records of older attempts are refused (audit A m1): never written, so never in a ledger.
        if attempt < j.attempt {
            return Err(invalid(format!(
                "job {job}: record of attempt {attempt}, the current attempt is {}",
                j.attempt
            )));
        }
        if new_attempt {
            if attempt == j.attempt && j.state == AttemptState::Queued {
                return Ok(()); // repeat of the record that created this attempt
            }
            if attempt != j.attempt + 1 {
                return Err(invalid(format!(
                    "job {job}: new attempt must be {}, got {attempt}",
                    j.attempt + 1
                )));
            }
            // audit B exp1: a new attempt only from a settled attempt that allows it (never from queued)
            let ok = match machine_ev {
                AttemptEvent::Requeue => {
                    matches!(j.state, AttemptState::Interrupted | AttemptState::Lost)
                }
                _ => matches!(j.state, AttemptState::Failed(_)),
            };
            if !ok {
                return Err(invalid(format!(
                    "job {job}: no new attempt from {} by {}",
                    j.state.tag(),
                    machine_ev.tag()
                )));
            }
        } else if attempt != j.attempt {
            return Err(invalid(format!(
                "job {job}: event for attempt {attempt}, current is {}",
                j.attempt
            )));
        }
        let next = apply_attempt(j.state, machine_ev).map_err(illegal)?;
        if next == j.state && !new_attempt {
            return Ok(()); // exact repeat
        }
        if let JE::Launch { .. } = ev {
            if let Some(i) = &j.stale {
                return Err(invalid(format!(
                    "launch of job {job}: input {i} changed since it was queued (stale)"
                )));
            }
            let it = &self.items[&j.item];
            // audit-ledger2 m5: a job that can never feed a result is never launched (economy, R31)
            if !matches!(it.phase, ItemPhase::Open | ItemPhase::InProgress) {
                return Err(invalid(format!(
                    "launch of job {job}: item {} is {} (its result could never be used)",
                    j.item,
                    it.phase.tag()
                )));
            }
            if j.generation != it.generation {
                return Err(invalid(format!(
                    "launch of job {job}: queued for generation {} of item {}, which is at generation {} (reopened since)",
                    j.generation, j.item, it.generation
                )));
            }
            if self.paused || it.paused || self.nodes[&it.node].paused {
                return Err(invalid(format!(
                    "launch of job {job}: paused by an operator control"
                )));
            }
        }
        if let JE::Finished {
            accept,
            binding,
            outcome,
            ..
        } = ev
            && accept_permits_records(*accept)
        {
            // The binding names the digest of the job.json THIS attempt was launched with (its launch record).
            let launched = j.attempt_spec.as_deref().unwrap_or("");
            match binding {
                Some(b)
                    if b.job == job
                        && b.attempt == attempt
                        && !launched.is_empty()
                        && b.spec_blake3 == launched => {}
                _ => {
                    return Err(invalid(format!(
                        "job {job} attempt {attempt}: accepting records needs the OUTCOME binding (job, attempt, launched spec digest {})",
                        short(launched)
                    )));
                }
            }
            if outcome.is_none() {
                return Err(invalid(format!(
                    "job {job} attempt {attempt}: accepting records needs the OUTCOME file reference"
                )));
            }
        }
        // UNCONFIRMED LAUNCH (execution unknown): the outcome must be the authenticated one of exactly this launch.
        let strict = self.strict_unconfirmed || ENFORCE_UNCONFIRMED_ACROSS_SESSIONS;
        let earlier_session = j.state == AttemptState::Launching
            && j.launch_session.is_some_and(|s| s < self.sessions);
        if strict
            && earlier_session
            && matches!(machine_ev, AttemptEvent::Interrupt | AttemptEvent::Lose)
        {
            return Err(invalid(format!(
                "job {job} attempt {attempt}: its launch (session {}) was never confirmed: execution unknown, never 'did not run' (mark it unconfirmed)",
                j.launch_session.unwrap_or(0)
            )));
        }
        if let JE::Finished {
            binding, outcome, ..
        } = ev
            && (j.state == AttemptState::Unconfirmed || (strict && earlier_session))
        {
            let launched = j.attempt_spec.as_deref().unwrap_or("");
            let bound = binding.as_ref().is_some_and(|b| {
                b.job == job
                    && b.attempt == attempt
                    && !launched.is_empty()
                    && b.spec_blake3 == launched
            });
            if !bound || outcome.is_none() {
                return Err(invalid(format!(
                    "job {job} attempt {attempt}: unconfirmed launch (execution unknown): a finish needs the authenticated OUTCOME of exactly this launch (binding job, attempt, launched spec digest {} + the OUTCOME file); never 'did not run'",
                    short(launched)
                )));
            }
        }
        if matches!(ev, JE::Cancel { .. }) && j.state == AttemptState::Unconfirmed {
            let it = &self.items[&j.item];
            let superseded = j.stale.is_some()
                || j.generation != it.generation
                || !matches!(it.phase, ItemPhase::Open | ItemPhase::InProgress);
            if !superseded {
                return Err(invalid(format!(
                    "cancel of job {job}: unconfirmed launch (execution unknown) of the item's current work: authenticate its run directory, or an operator retries item {} first",
                    j.item
                )));
            }
        }
        let item = j.item.clone();
        let rung = j.rung;
        let session = self.sessions;
        let j = self.jobs.get_mut(job).unwrap();
        j.state = next;
        if new_attempt {
            j.attempt = attempt;
            j.run_dir = None;
            j.pid = None;
            j.start_ticks = None;
            j.boot_id = None;
            j.sealed_table = None;
            j.outcome = None;
            j.accept = None;
            j.time_decided = false;
            j.launch_session = None;
            j.prev_attempt_spec = j.attempt_spec.take();
        }
        match ev {
            JE::Launch {
                run_dir,
                spec_blake3,
                ..
            } => {
                j.run_dir = Some(run_dir.clone());
                j.attempt_spec = Some(spec_blake3.clone());
                j.launch_session = Some(session);
            }
            JE::Unconfirmed { reason } => {
                let text = format!(
                    "job {} attempt {attempt}: launch never confirmed, execution unknown ({reason})",
                    short(job)
                );
                self.push_recent(rec, "launch_unconfirmed", text);
            }
            JE::NotSpawned { reason } => {
                let text = format!(
                    "job {} attempt {attempt}: unconfirmed launch proven never spawned ({reason})",
                    short(job)
                );
                self.push_recent(rec, "launch_not_spawned", text);
            }
            JE::Started {
                pid,
                start_ticks,
                boot_id,
            } => {
                j.pid = Some(*pid);
                j.start_ticks = Some(*start_ticks);
                j.boot_id = Some(boot_id.clone());
            }
            JE::Finished {
                class,
                accept,
                sealed_output,
                outcome,
                cpu_ms,
                peak_rss_bytes,
                time_decided,
                ..
            } => {
                j.sealed_table = sealed_output.clone();
                j.outcome = outcome.clone();
                j.accept = Some(*accept);
                j.time_decided = *time_decided;
                j.cpu_ms += *cpu_ms;
                j.peak_rss_bytes = j.peak_rss_bytes.max(*peak_rss_bytes);
                let failed = matches!(next, AttemptState::Failed(_));
                if failed {
                    j.failures += 1;
                }
                self.cpu_ms_total += *cpu_ms;
                if !matches!(class, FinishClass::Interrupted | FinishClass::Lost) {
                    self.costs_ms.entry(rung).or_default().push(*cpu_ms);
                }
                if let Some(it) = self.items.get_mut(&item) {
                    it.cpu_ms += *cpu_ms;
                    if failed {
                        it.failed_attempts += 1;
                    }
                    let node = it.node.clone();
                    let label = it.label.clone();
                    if let Some(n) = self.nodes.get_mut(&node) {
                        n.cpu_ms += *cpu_ms;
                        n.state.cpu_ms_on_target += *cpu_ms;
                    }
                    self.push_recent(
                        rec,
                        "job_finished",
                        format!(
                            "{node} {label} {rung}: job {} ({}, accept {})",
                            short(job),
                            class_word(*class),
                            accept_word(*accept)
                        ),
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn apply_control(
        &mut self,
        rec: &Record,
        id: &str,
        ev: &ControlEvent,
    ) -> Result<(), ReduceError> {
        let seq = rec.seq;
        let invalid = |why: String| ReduceError::Invalid { seq, why };
        match ev {
            ControlEvent::Requested {
                verb,
                target,
                by,
                reason,
            } => {
                if let Some(c) = self.controls.get(id) {
                    if c.verb != *verb || c.target != *target || c.by != *by {
                        return Err(invalid(format!(
                            "control {id} re-requested with other content"
                        )));
                    }
                    return Ok(());
                }
                match target {
                    ControlTarget::Campaign => {}
                    ControlTarget::Node { node } if self.nodes.contains_key(node) => {}
                    ControlTarget::Item { item } if self.items.contains_key(item) => {}
                    _ => return Err(invalid(format!("control {id}: unknown target {target:?}"))),
                }
                let ok = matches!(
                    (verb, target),
                    (ControlVerb::Pause | ControlVerb::Resume, _)
                        | (ControlVerb::Stop, ControlTarget::Campaign)
                        | (
                            ControlVerb::Retry | ControlVerb::Skip,
                            ControlTarget::Item { .. }
                        )
                );
                if !ok {
                    return Err(invalid(format!(
                        "control {id}: {verb:?} does not apply to {target:?}"
                    )));
                }
                self.controls.insert(
                    id.to_string(),
                    ControlRec {
                        verb: *verb,
                        target: target.clone(),
                        by: by.clone(),
                        reason: reason.clone(),
                        requested_seq: seq,
                        requested_utc: rec.utc.clone(),
                        status: "requested".into(),
                        outcome: None,
                        done_seq: None,
                        done_utc: None,
                    },
                );
                self.push_recent(
                    rec,
                    "control",
                    format!("{by} requests {verb:?} of {target:?} ({reason})"),
                );
            }
            ControlEvent::Refused { why } => {
                let c = self
                    .controls
                    .get(id)
                    .ok_or_else(|| invalid(format!("control {id} was never requested")))?;
                match c.status.as_str() {
                    "refused" if c.outcome.as_deref() == Some(why) => return Ok(()),
                    "requested" => {}
                    s => return Err(invalid(format!("control {id} is already {s}"))),
                }
                let c = self.controls.get_mut(id).unwrap();
                c.status = "refused".into();
                c.outcome = Some(why.clone());
                c.done_seq = Some(seq);
                c.done_utc = Some(rec.utc.clone());
                self.push_recent(rec, "control", format!("control {id} refused: {why}"));
            }
            ControlEvent::Applied { effect } => {
                let c = self
                    .controls
                    .get(id)
                    .ok_or_else(|| invalid(format!("control {id} was never requested")))?
                    .clone();
                match c.status.as_str() {
                    "applied" => return Ok(()),
                    "requested" => {}
                    s => return Err(invalid(format!("control {id} is already {s}"))),
                }
                match (&c.verb, &c.target) {
                    (ControlVerb::Pause | ControlVerb::Resume, t) => {
                        let v = c.verb == ControlVerb::Pause;
                        match t {
                            ControlTarget::Campaign => self.paused = v,
                            ControlTarget::Node { node } => {
                                self.nodes.get_mut(node).unwrap().paused = v
                            }
                            ControlTarget::Item { item } => {
                                self.items.get_mut(item).unwrap().paused = v
                            }
                        }
                    }
                    (ControlVerb::Stop, _) => {}
                    (ControlVerb::Retry | ControlVerb::Skip, ControlTarget::Item { item }) => {
                        let mev = if c.verb == ControlVerb::Retry {
                            ItemMachineEvent::ControlRetry
                        } else {
                            ItemMachineEvent::ControlSkip
                        };
                        let it = &self.items[item];
                        let cur = ItemState {
                            phase: it.phase.clone(),
                            rung: it.rung,
                        };
                        let next = apply_item(&cur, &mev)
                            .map_err(|err| ReduceError::Illegal { seq, err })?;
                        if c.verb == ControlVerb::Retry
                            && it.phase == ItemPhase::Stale
                            && self.nodes[&it.node].state.phase == NodePhase::Stale
                        {
                            return Err(invalid(format!(
                                "retry of {item}: node {} is stale (reopen the node first)",
                                it.node
                            )));
                        }
                        // audit-ledger2 m4: as item reopen (audit A m11), never revive an item whose child
                        // carries its work
                        if c.verb == ControlVerb::Retry
                            && next != cur
                            && let Some(ch) = self.live_child_of(item)
                        {
                            return Err(invalid(format!(
                                "retry of {item}: its child {ch} carries the work"
                            )));
                        }
                        if next != cur {
                            let node_id = it.node.clone();
                            let it = self.items.get_mut(item).unwrap();
                            let from = std::mem::replace(&mut it.phase, next.phase.clone());
                            it.updated_seq = seq;
                            if next.phase == ItemPhase::Open {
                                it.generation += 1;
                                it.produced = None;
                                it.operator_retries += 1;
                                if from == ItemPhase::Stale {
                                    it.stale_from = None;
                                    it.stale_input = None;
                                    it.accepted = None;
                                    it.exclusions.clear();
                                }
                            }
                            it.attention = None;
                            let nr = self.nodes.get_mut(&node_id).unwrap();
                            nr.items_by_phase[from.index()] -= 1;
                            nr.items_by_phase[next.phase.index()] += 1;
                            self.recheck_node(
                                rec,
                                &node_id,
                                &format!("operator {:?} of {item}", c.verb),
                                next.phase == ItemPhase::Open,
                            );
                        }
                    }
                    _ => return Err(invalid(format!("control {id}: verb does not apply"))),
                }
                let cr = self.controls.get_mut(id).unwrap();
                cr.status = "applied".into();
                cr.outcome = Some(effect.clone());
                cr.done_seq = Some(seq);
                cr.done_utc = Some(rec.utc.clone());
                self.push_recent(rec, "control", format!("control {id} applied: {effect}"));
            }
        }
        Ok(())
    }

    fn push_recent(&mut self, rec: &Record, kind: &str, text: String) {
        if self.recent.len() == RECENT_CAP {
            self.recent.pop_front();
        }
        self.recent.push_back(RecentEvent {
            seq: rec.seq,
            utc: rec.utc.clone(),
            kind: kind.to_string(),
            text,
        });
    }

    /// Economy report of a node (R31, U-13): `before` = every rule ever accepted or imported (still live or
    /// retired / superseded since), `after` = the live rules (what ships).
    pub fn node_economy(&self, node: &str) -> NodeEconomy {
        let mut e = NodeEconomy::default();
        for it in self.items.values().filter(|i| i.node == node) {
            let live = matches!(it.phase, ItemPhase::Accepted(_));
            let was = live
                || it.phase == ItemPhase::Retired
                || (it.phase == ItemPhase::Stale && it.stale_from.as_deref() == Some("retired"));
            if !was {
                continue;
            }
            e.rules_before += 1;
            if live {
                e.rules_after += 1;
            }
            match &it.economy {
                Some(m) => {
                    e.before.add(m);
                    if live {
                        e.after.add(m);
                    }
                }
                None => e.unmeasured += 1,
            }
            if it
                .retired
                .as_ref()
                .is_some_and(|r| r.superseded_by.is_some())
            {
                e.superseded += 1;
            }
        }
        e
    }

    /// Terminal report of a node (R41 P-4): preferred used / not needed / other.
    pub fn terminal_report(&self, node: &str) -> TerminalReport {
        let Some(n) = self.nodes.get(node) else {
            return TerminalReport::default();
        };
        let mut r = TerminalReport {
            preferred_listed: n.state.spec.preferred_listed,
            counted: n.state.terminals as u64,
            bound: n.state.terminal_bound,
            ..Default::default()
        };
        for it in self
            .items
            .values()
            .filter(|i| i.node == node && i.phase == ItemPhase::Terminal)
        {
            if it.terminal_preferred {
                r.preferred_used += it.terminal_keys as u64;
            } else {
                r.other += it.terminal_keys as u64;
            }
        }
        r.preferred_not_needed = r
            .preferred_listed
            .map(|l| l.saturating_sub(r.preferred_used));
        r
    }

    /// [`stamp_digest`] of the CURRENT digests of `ids` (Err: the first unknown id).
    pub fn stamp_digest(&self, ids: &[String]) -> Result<String, String> {
        let mut h = blake3::Hasher::new();
        for i in ids {
            let r = self.inputs.get(i).ok_or_else(|| i.clone())?;
            h.update(i.as_bytes());
            h.update(&[0]);
            h.update(r.blake3.as_bytes());
            h.update(&[1]);
        }
        Ok(h.finalize().to_hex().to_string())
    }

    /// The node's current rule-set digest (what a closure result must cite).
    pub fn node_rule_set_digest(&self, node: &str) -> Option<String> {
        self.nodes.get(node).map(|n| n.rule_set.digest())
    }

    /// The node's live rules `(item, rule, record_blake3, exclusions)` (input of [`rule_set_digest_of`]).
    pub fn node_live_rules(&self, node: &str) -> Vec<(String, String, String, Vec<String>)> {
        self.items
            .iter()
            .filter(|(_, it)| it.node == node && matches!(it.phase, ItemPhase::Accepted(_)))
            .filter_map(|(id, it)| {
                it.accepted.as_ref().map(|a| {
                    (
                        id.clone(),
                        a.rule.clone(),
                        a.record.blake3.clone(),
                        it.exclusions.clone(),
                    )
                })
            })
            .collect()
    }

    /// Attempts that may still have a process (launching / running): what a resuming supervisor must settle.
    pub fn unsettled_jobs(&self) -> Vec<(&String, &JobRec)> {
        self.jobs
            .iter()
            .filter(|(_, j)| !j.state.is_settled())
            .collect()
    }

    /// Items with phase counts over the whole campaign, indexed like [`ItemPhase::TAGS`].
    pub fn items_by_phase(&self) -> [u64; N_ITEM_PHASES] {
        let mut out = [0; N_ITEM_PHASES];
        for n in self.nodes.values() {
            for (o, v) in out.iter_mut().zip(n.items_by_phase) {
                *o += v;
            }
        }
        out
    }

    /// Stable digest of the state (blake3 of its JSON): used to compare snapshot + tail with full replay.
    pub fn digest(&self) -> String {
        // streamed into the hasher: no serialised copy of the state in memory (2M records: ~0.4 GB)
        let mut h = blake3::Hasher::new();
        serde_json::to_writer(&mut h, self).expect("state serialises");
        h.finalize().to_hex().to_string()
    }

    /// The non-stale child of `item` that carries its work, if any (via the children index).
    pub fn live_child_of(&self, item: &str) -> Option<&String> {
        self.items.get(item)?.children.iter().find(|c| {
            self.items
                .get(c.as_str())
                .is_some_and(|c| c.phase != ItemPhase::Stale)
        })
    }

    /// A job of `item`'s CURRENT generation whose launch is unconfirmed (execution unknown), if any.
    pub fn unconfirmed_job_of(&self, item: &str) -> Option<&String> {
        let it = self.items.get(item)?;
        it.jobs.iter().find(|j| {
            self.jobs.get(j.as_str()).is_some_and(|j| {
                j.state == AttemptState::Unconfirmed && j.generation == it.generation
            })
        })
    }

    /// THE RECOVERY HOOK's worklist: attempts still `launching` whose launch intent was journalled in an EARLIER
    /// session (a `campaign start` came after it) and never confirmed by `job.started`. A resuming supervisor
    /// appends `job.unconfirmed {reason}` for each (one group commit, before anything else is decided about them),
    /// then authenticates each run directory (see [`AttemptState::Unconfirmed`]).
    pub fn unconfirmed_launches(&self) -> Vec<(&String, &JobRec)> {
        self.jobs
            .iter()
            .filter(|(_, j)| {
                j.state == AttemptState::Launching
                    && j.launch_session.is_some_and(|s| s < self.sessions)
            })
            .collect()
    }

    /// Attempts whose execution is unknown (state `unconfirmed`): each needs its run directory authenticated (or,
    /// after an operator retry of its item, a `cancel`).
    pub fn execution_unknown_jobs(&self) -> Vec<(&String, &JobRec)> {
        self.jobs
            .iter()
            .filter(|(_, j)| j.state == AttemptState::Unconfirmed)
            .collect()
    }

    /// M2: the first input of `it` (its own, its node's, its node's spec box) whose digest CHANGED after `seq`
    /// (`(input, seq of the change)`): evidence or tables recorded before that change were made under the old input.
    pub fn input_changed_after(&self, seq: u64, it: &ItemRec) -> Option<(String, u64)> {
        let n = self.nodes.get(&it.node)?;
        it.inputs
            .iter()
            .chain(n.inputs.iter())
            .chain(n.state.spec_box.iter())
            .filter_map(|i| self.inputs.get(i).map(|r| (i, r)))
            .find(|(_, r)| r.versions > 1 && seq < r.registered_seq)
            .map(|(i, r)| (i.clone(), r.registered_seq))
    }
}

/// M3: a level above geometry needs the matching artefacts.
fn check_level_evidence(
    level: EvidenceLevel,
    evidence: Option<&crate::ledger::LevelEvidence>,
) -> Result<(), String> {
    match (level, evidence) {
        (EvidenceLevel::GeometryAccepted, None) => Ok(()),
        (EvidenceLevel::GeometryAccepted, Some(_)) => {
            Err("geometry_accepted carries no identity evidence (use the matching level)".into())
        }
        (l, None) => Err(format!(
            "level {} needs its evidence artefacts (test verdict / per-key proofs)",
            l.word()
        )),
        (l, Some(e)) if e.level() != l => Err(format!(
            "level {} does not match the cited evidence ({})",
            l.word(),
            e.level().word()
        )),
        _ => Ok(()),
    }
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

    const ACCEPTS: [ContractAccept; 4] = [
        ContractAccept::Canonical,
        ContractAccept::CommittedRules,
        ContractAccept::SolvedRecords,
        ContractAccept::Nothing,
    ];

    fn all_attempt_states() -> Vec<AttemptState> {
        let mut v = vec![
            AttemptState::Queued,
            AttemptState::Launching,
            AttemptState::Running,
            AttemptState::Interrupted,
            AttemptState::Lost,
            AttemptState::Cancelled,
        ];
        for c in FINISH_CLASSES {
            match class_target(c) {
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
            AttemptEvent::Unconfirmed,
            AttemptEvent::NotSpawned,
        ];
        for c in FINISH_CLASSES {
            for a in ACCEPTS {
                for sealed in [true, false] {
                    v.push(AttemptEvent::Finished {
                        class: c,
                        accept: a,
                        sealed,
                    });
                }
            }
        }
        v
    }

    fn table_has(table: &[(&str, &str, &str)], from: &str, ev: &str) -> bool {
        table
            .iter()
            .any(|(f, e, _)| *e == ev && (f.split('|').any(|x| x == from) || f.starts_with("<any")))
    }

    #[test]
    fn contract_mapping_is_the_decision_table() {
        // every class allows at least one value, and only the record-producing rows allow records
        for c in FINISH_CLASSES {
            assert!(!contract_accepts(c).is_empty());
        }
        let records: Vec<FinishClass> = FINISH_CLASSES
            .into_iter()
            .filter(|c| {
                contract_accepts(*c)
                    .iter()
                    .any(|a| accept_permits_records(*a))
            })
            .collect();
        assert_eq!(
            records,
            vec![
                FinishClass::RuleFound,
                FinishClass::Complete,
                FinishClass::NoRule,
                FinishClass::Spill
            ]
        );
    }

    #[test]
    fn attempt_machine_matches_its_table_exhaustively() {
        for s in all_attempt_states() {
            for e in all_attempt_events() {
                match apply_attempt(s, e) {
                    Ok(n) if n == s => {}
                    Ok(_) => assert!(
                        table_has(ATTEMPT_TABLE, s.tag(), e.tag()),
                        "{s:?} --{e:?}--> not in table"
                    ),
                    Err(_) => {
                        if table_has(ATTEMPT_TABLE, s.tag(), e.tag()) {
                            let AttemptEvent::Finished {
                                class,
                                accept,
                                sealed,
                            } = e
                            else {
                                panic!("{s:?} {e:?}");
                            };
                            assert!(
                                !contract_accepts(class).contains(&accept)
                                    || (accept_permits_records(accept) && !sealed),
                                "{s:?} {e:?}"
                            );
                        }
                    }
                }
            }
        }
        for s in all_attempt_states()
            .into_iter()
            .filter(|s| s.is_settled() && *s != AttemptState::Queued)
        {
            for e in all_attempt_events() {
                if let Ok(n) = apply_attempt(s, e) {
                    assert!(n.is_settled(), "{s:?} -> {n:?}");
                }
            }
        }
        // records-accepting needs a seal; a checker verdict without records needs none
        assert!(
            apply_attempt(
                AttemptState::Running,
                AttemptEvent::Finished {
                    class: FinishClass::RuleFound,
                    accept: ContractAccept::Canonical,
                    sealed: false
                }
            )
            .is_err()
        );
        assert!(
            apply_attempt(
                AttemptState::Running,
                AttemptEvent::Finished {
                    class: FinishClass::DecidedNegative,
                    accept: ContractAccept::Nothing,
                    sealed: false
                }
            )
            .is_ok()
        );
        assert!(
            apply_attempt(
                AttemptState::Running,
                AttemptEvent::Finished {
                    class: FinishClass::DecidedNegative,
                    accept: ContractAccept::Canonical,
                    sealed: true
                }
            )
            .is_err()
        );
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
        let mut v = vec![
            E::Start,
            E::Produce,
            E::Requeue,
            E::ProposeTerminal,
            E::DeclareTerminal,
            E::MarkStale,
            E::Reopen,
            E::Retire,
            E::ControlRetry,
            E::ControlSkip,
            E::Attention("needs_bigmem".into()),
        ];
        for l in EvidenceLevel::ALL {
            v.push(E::Accept(l));
            v.push(E::Import(l));
            v.push(E::Upgrade(l));
            v.push(E::Reinstate(l));
        }
        v.push(E::Reject("cert_failed".into()));
        v.push(E::Reject("rule_covers_none".into()));
        v.push(E::Escalate(Rung(0)));
        v.push(E::Escalate(Rung(1)));
        v.push(E::Escalate(Rung(3)));
        v
    }

    fn item_phases() -> Vec<ItemPhase> {
        let mut v = vec![
            ItemPhase::Open,
            ItemPhase::InProgress,
            ItemPhase::Produced,
            ItemPhase::TerminalCandidate,
            ItemPhase::Terminal,
            ItemPhase::Stale,
            ItemPhase::Retired,
            ItemPhase::Skipped,
            ItemPhase::Attention("needs_bigmem".into()),
        ];
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
            let s = ItemState {
                phase: p.clone(),
                rung: Rung(1),
            };
            for e in item_events() {
                if let Ok(n) = apply_item(&s, &e)
                    && n != s
                {
                    assert!(
                        table_has(ITEM_TABLE, s.phase.tag(), e.tag()),
                        "{p:?} --{e:?}--> {n:?} not in table"
                    );
                    if let ItemMachineEvent::Escalate(r) = e {
                        assert!(r > s.rung);
                    }
                    if let (ItemPhase::Accepted(a), ItemPhase::Accepted(b)) = (&s.phase, &n.phase) {
                        assert!(b > a, "level decreased");
                    }
                }
            }
        }
        for p in [ItemPhase::Terminal, ItemPhase::Escalated(Rung(2))] {
            let s = ItemState {
                phase: p,
                rung: Rung(1),
            };
            for e in item_events() {
                if let Ok(n) = apply_item(&s, &e) {
                    assert!(n == s || n.phase == ItemPhase::Stale);
                }
            }
        }
    }

    fn ctx_with(digest: &str) -> NodeContext {
        NodeContext {
            rule_set_digest: digest.into(),
            ..Default::default()
        }
    }

    fn cr(round: u32, defect: u64, rs: &str) -> NodeEvent {
        NodeEvent::ClosureResult {
            round,
            defect_keys: defect,
            closure_keys: 100,
            rules_used: Some(7),
            rules_total: Some(70),
            keyset: None,
            rule_set_digest: rs.into(),
            dispatch_order_digest: "order".into(),
            envelope: EnvelopeKind::Proved,
            envelope_tables: vec![],
        }
    }

    fn assembled() -> NodeEvent {
        let f = |p: &str| FileRef {
            path: p.into(),
            bytes: 1,
            blake3: p.into(),
        };
        NodeEvent::Assembled {
            round: 1,
            manifest: f("manifest"),
            dispatch_order: f("order"),
        }
    }

    #[test]
    fn node_verdict_is_bound_to_the_rule_set() {
        let ctx = ctx_with("rs1");
        let s = apply_node(
            &NodeState::new(None, 2, NodeSpec::default()),
            &NodeEvent::Start,
            &ctx,
        )
        .unwrap();
        assert!(
            apply_node(&s, &cr(1, 0, "rs1"), &ctx).is_err(),
            "no dispatch order"
        );
        let s = apply_node(&s, &assembled(), &ctx).unwrap();
        assert!(
            apply_node(&s, &cr(1, 0, "rs0"), &ctx).is_err(),
            "older rule set"
        );
        let s = apply_node(&s, &cr(1, 0, "rs1"), &ctx).unwrap();
        assert!(
            apply_node(&s, &NodeEvent::DeclareClosed, &ctx_with("rs2")).is_err(),
            "rule set changed since the closure"
        );
        let open1 = NodeContext {
            items_open: 1,
            ..ctx.clone()
        };
        assert!(apply_node(&s, &NodeEvent::DeclareClosed, &open1).is_err());
        let c = apply_node(&s, &NodeEvent::DeclareClosed, &ctx).unwrap();
        assert_eq!((c.phase, c.closure_kind()), (NodePhase::Closed, "full"));
        assert!(c.full_closed_ever);
        // FULL default certification = replay: rules below identity_tested block
        let low = NodeContext {
            below_tested: 1,
            below_proved: 3,
            ..ctx.clone()
        };
        assert!(apply_node(&c, &NodeEvent::DeclareCertified, &low).is_err());
        let ok = NodeContext {
            below_tested: 0,
            below_proved: 3,
            ..ctx.clone()
        };
        assert!(
            apply_node(&c, &NodeEvent::DeclareCertified, &ok)
                .unwrap()
                .certified()
        );
        // internal events
        let t = apply_node_internal(&c, NodeInternal::TerminalDeclared { count: 2 }).unwrap();
        assert!(apply_node_internal(&t, NodeInternal::TerminalDeclared { count: 1 }).is_err());
        assert!(apply_node_internal(&c, NodeInternal::Retirement { box_only: true }).is_err());
        assert!(apply_node_internal(&c, NodeInternal::Retirement { box_only: false }).is_ok());
    }

    #[test]
    fn full_first_then_spec_box_or_open() {
        let ctx = ctx_with("rs");
        let blk = Blockers {
            faces: 3,
            loci: 1,
            keys: 87,
            list: None,
        };
        let spec = NodeSpec {
            full_attempt_fixer_rounds: 1,
            ..NodeSpec::default()
        };
        let ended = |s: &NodeState| {
            apply_node(
                s,
                &NodeEvent::AttemptEnded {
                    blockers: blk.clone(),
                    reason: "budget".into(),
                    cause: AttemptEndCause::FixerRounds,
                },
                &ctx,
            )
        };
        let run = |s: NodeState| {
            let s = apply_node(&s, &NodeEvent::Start, &ctx).unwrap();
            let s = apply_node(&s, &assembled(), &ctx).unwrap();
            assert!(ended(&s).is_err(), "attempt never ran");
            let s = apply_node(&s, &cr(1, 5, "rs"), &ctx).unwrap();
            assert!(ended(&s).is_err(), "budget not exhausted");
            apply_node(
                &s,
                &NodeEvent::FixerRoundBegin {
                    round: 1,
                    partition: None,
                },
                &ctx,
            )
            .unwrap()
        };
        let fb = |id: &str| NodeEvent::FallBackToBox { box_id: id.into() };
        let s = run(NodeState::new(None, 100, spec.clone()));
        assert!(
            apply_node(&s, &fb("box"), &ctx).is_err(),
            "BOXED without a spec box"
        );
        let s = ended(&s).unwrap();
        let o = apply_node(&s, &NodeEvent::DeclareOpen { reason: "x".into() }, &ctx).unwrap();
        assert_eq!((o.phase, o.closure_kind()), (NodePhase::Open, "open"));
        let s = run(NodeState::new(Some("box".into()), 100, spec.clone()));
        assert!(
            apply_node(&s, &fb("box"), &ctx).is_err(),
            "full attempt first"
        );
        let s = ended(&s).unwrap();
        assert!(apply_node(&s, &fb("wider"), &ctx).is_err());
        let b = apply_node(&s, &fb("box"), &ctx).unwrap();
        assert!(b.closure.is_none() && b.fixer_rounds_on_target == 0);
        let b = apply_node(&b, &cr(2, 0, "rs"), &ctx).unwrap();
        let c = apply_node(&b, &NodeEvent::DeclareClosed, &ctx).unwrap();
        assert_eq!(c.closure_kind(), "boxed");
        assert_eq!(c.required_cert(), CertLevel::PerKey);
        // a node closed FULL once is never boxed
        let mut f = s.clone();
        f.full_closed_ever = true;
        assert!(apply_node(&f, &fb("box"), &ctx).is_err());
    }

    #[test]
    fn campaign_machine() {
        use CampaignEvent as E;
        let p = apply_campaign(None, E::Start).unwrap();
        assert!(apply_campaign(None, E::Run).is_err());
        let p = apply_campaign(Some(p), E::Run).unwrap();
        let d = apply_campaign(Some(p), E::Drain).unwrap();
        assert!(apply_campaign(Some(d), E::Finish).is_err());
        let st = apply_campaign(Some(d), E::Stop).unwrap();
        assert_eq!(
            apply_campaign(Some(st), E::Start).unwrap(),
            CampaignPhase::Starting
        );
        for s in CampaignPhase::ALL {
            for e in [E::Start, E::Run, E::Drain, E::Stop, E::Finish] {
                if let Ok(n) = apply_campaign(Some(s), e) {
                    assert!(
                        n == s || table_has(CAMPAIGN_TABLE, s.tag(), e.tag()),
                        "{s:?} {e:?}"
                    );
                }
            }
        }
        let ctx = NodeContext::default();
        let base = NodeState::new(None, 2, NodeSpec::default());
        for s in NodePhase::ALL {
            let st = NodeState {
                phase: s,
                ..base.clone()
            };
            for e in [
                NodeEvent::Start,
                NodeEvent::DeclareClosed,
                NodeEvent::DeclareCertified,
                NodeEvent::Reopen { reason: "x".into() },
                NodeEvent::DeclareOpen { reason: "x".into() },
                NodeEvent::FallBackToBox { box_id: "b".into() },
            ] {
                if let Ok(n) = apply_node(&st, &e, &ctx) {
                    assert!(
                        n == st
                            || NODE_TABLE.iter().any(|(f, ev, _)| *ev == e.tag()
                                && (f.split('|').any(|x| x == s.tag()) || f.starts_with("<any"))),
                        "{s:?} {e:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn rule_set_accumulator_equals_from_scratch() {
        let mut acc = RuleSetAcc::default();
        let ex: Vec<String> = vec![];
        let a = rule_element("i1", "r1", "b1", &ex);
        let b = rule_element("i2", "r2", "b2", &ex);
        acc.add(a);
        acc.add(b);
        assert_eq!(
            acc.digest(),
            rule_set_digest_of([("i2", "r2", "b2", &ex[..]), ("i1", "r1", "b1", &ex[..])])
        );
        acc.sub(a);
        assert_eq!(
            acc.digest(),
            rule_set_digest_of([("i2", "r2", "b2", &ex[..])])
        );
        acc.sub(b);
        assert_eq!(acc, RuleSetAcc::default());
    }
}
