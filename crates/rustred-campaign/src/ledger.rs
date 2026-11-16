//! The LEDGER OF RECORD of a campaign: append-only JSONL, schema `rustred.campaign.ledger.v1`, one record per
//! state change, hash-chained, self-checking per line, segmented with a manifest, snapshots and a durable
//! commit mark.
//!
//! Layout (inside the campaign directory CDIR; nothing of this module writes anywhere else):
//! ```text
//! CDIR/ledger/MANIFEST.json            rustred.campaign.ledger-manifest.v1 (atomic; the commit point of rotation)
//! CDIR/ledger/COMMIT                   commit mark: two 256-byte slots, (segment, seq, offset, head) acknowledged durable
//! CDIR/ledger/segment-000001.jsonl     sealed segments: read-only (0444), BLAKE3 + last hash in the manifest
//! CDIR/ledger/segment-000002.jsonl     the active segment (append-only)
//! CDIR/ledger/snapshot-000000004711.json  rustred.campaign.snapshot.v1: reduced state after record 4711
//! CDIR/ledger/torn-<seq>-<utc ms>.bin  bytes dropped by a torn-tail recovery or an operator override (forensics)
//! CDIR/ledger/RECOVERY-PENDING.json    present only between a truncation and its `recovery` record (crash window)
//! ```
//!
//! RECORD LINE (audit A C4, m8): `<body without its closing brace>,"h":"<hex blake3 of body>"}` where `body` is
//! the canonical serialisation of [`Record`] (`{"schema":..,"seq":N,"utc":..,"prev":"<h of the previous record>",
//! "event":{"kind":..,"body":{..}}}`; fixed field order, no floats). Every reader checks, for every line: the
//! suffix framing, the digest, that the body re-serialises to exactly the same bytes (canonical form: an edit
//! inside a string, an extra field or a different spelling breaks it), the schema, the dense `seq` and the `prev`
//! chain. The chain value ("head") is the `h` of the last record.
//!
//! COMMIT MARK: after every successful fsync of the active segment the ledger writes the mark (alternating slots,
//! each with its own checksum) and fsyncs it. On open, bytes after the last valid record are a TORN TAIL only if
//! they lie beyond the mark AND are an incomplete line (no newline) or contain NUL bytes (zero-filled blocks after
//! a power loss) AND no complete valid record follows. Anything else (a complete line failing its digest, damage
//! at or before the mark, a mark beyond the end of the data) is CORRUPTION: `open` refuses with a typed error.
//! The operator override [`OpenOptions::accept_truncation_to`] truncates to a named seq, saves the dropped bytes
//! and writes a `recovery` record with `forced: true`.
//!
//! FSYNC POLICY ([`Event::durability`]): `Sync` records are fsynced before `append` returns, then the commit mark
//! is written and fsynced; `Lazy` records are written at once (one positioned write per batch, so they survive a
//! process kill) and fsynced with the next `Sync` record, after `lazy_max_records`, after `lazy_max_ms`, or by
//! [`Ledger::sync`]. The exact operation order is tested through the [`FileOps`] seam (tests/ledger_fsync_policy.rs).
//!
//! FAILURES (audit A M1, M2, m3): any failed write or fsync of the active segment POISONS the handle: the batch is
//! truncated away (best effort), the in-memory state is rebuilt from disk without it, every later call returns
//! `Poisoned`, and the caller must drop the handle and `open` again. FSYNC GATE: `open` rewrites every byte beyond
//! the commit mark (making pages a failed fsync left "clean but not on disk" dirty again) and fsyncs before
//! returning, so nothing read back from the page cache is acknowledged without a successful fsync. A failure of the
//! ROTATION after a durable append does not fail the append: the records are acknowledged, the failure is kept as a
//! warning ([`Ledger::warnings`]) and the rotation is retried later; a failure after the manifest rename (where the
//! on-disk manifest may already name the new segment) poisons the handle.
//!
//! WRITERS (audit A M6): a write handle needs a held [`CampaignLock`] of the same CDIR and takes an exclusive flock
//! on the ledger directory for its lifetime (a second write handle, even in the same process, is refused).
//! Dashboards, the website and `verify` use [`open_read_only`] / [`verify_chain`], which never write.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions as FsOpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{FileExt, PermissionsExt};
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::atomic::{self, AtomicError};
use crate::clock::Clock;
use crate::lock::{CampaignLock, DeadReason, OwnerRecord, ProcessIdentity};
use crate::state::{
    CampaignEvent, CampaignState, CertLevel, ContractAccept, EvidenceLevel, FinishClass, NodeEvent,
    ReduceError, Rung,
};

pub const LEDGER_SCHEMA: &str = "rustred.campaign.ledger.v1";
pub const MANIFEST_SCHEMA: &str = "rustred.campaign.ledger-manifest.v1";
/// v2 (lane ledgerfix2): the reduced state gained the parent -> children index and the launch session of each
/// attempt. A v1 snapshot named by an older manifest is IGNORED (full replay of the segments; the next rotation
/// writes a v2 snapshot); a snapshot is a cache, the segments are the truth.
pub const SNAPSHOT_SCHEMA: &str = "rustred.campaign.snapshot.v2";
/// Schema prefix shared by every snapshot version (an older version is recognised and ignored, never misread).
const SNAPSHOT_SCHEMA_FAMILY: &str = "rustred.campaign.snapshot.";
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const LEDGER_DIR: &str = "ledger";
pub const MANIFEST_FILE: &str = "MANIFEST.json";
pub const COMMIT_FILE: &str = "COMMIT";
pub const RECOVERY_PENDING_FILE: &str = "RECOVERY-PENDING.json";
/// Upper bound of one record line (large payloads go to files referenced by digest).
pub const MAX_RECORD_BYTES: usize = 64 * 1024;
/// Size of one commit-mark slot (the file holds two).
pub const MARK_SLOT_BYTES: usize = 256;
/// Bytes of the per-line suffix `,"h":"<64 hex>"}`.
const SUFFIX_LEN: usize = 6 + 64 + 2;

// ------------------------------------------------------------------------------------------------------------
// Record schema
// ------------------------------------------------------------------------------------------------------------

/// A file named by a record: path relative to CDIR (or `large:`-prefixed for the non-snapshotted large root of
/// the spec), size and BLAKE3. Key sets, tables and certificates are always referenced this way, never inlined.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FileRef {
    pub path: String,
    pub bytes: u64,
    pub blake3: String,
}

/// Where an accepted record comes from: a sealed table of a successful attempt, or imported hand-made evidence.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "from", rename_all = "snake_case")]
pub enum AcceptSource {
    Job {
        job: String,
        attempt: u32,
        table_blake3: String,
    },
    Evidence {
        evidence: String,
    },
}

/// The artefacts that justify an evidence level above geometry (audit A M3): `identity_tested` (= certification
/// level `replay`: identity replay at two primes distinct from the reconstruction primes, descent, guard / pole
/// checks) cites the test verdict record; `identity_proved` (= `per-key`) cites the per-key proof artefact and the
/// key set it covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "level", rename_all = "snake_case")]
pub enum LevelEvidence {
    IdentityTested { verdict: FileRef },
    IdentityProved { proofs: FileRef, keys: FileRef },
}

impl LevelEvidence {
    pub fn level(&self) -> EvidenceLevel {
        match self {
            LevelEvidence::IdentityTested { .. } => EvidenceLevel::IdentityTested,
            LevelEvidence::IdentityProved { .. } => EvidenceLevel::IdentityProved,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum ItemEvent {
    Start {
        job: Option<String>,
    },
    /// The attempt's sealed table holds a candidate rule for this item.
    Produce {
        job: String,
        attempt: u32,
        table: FileRef,
    },
    /// `record` = the accepted rule record, written (atomically, content-addressed) BEFORE this line.
    /// `evidence` is required for (and must match) any level above `geometry_accepted`; `round` = fixer round.
    Accept {
        rule: String,
        level: EvidenceLevel,
        record: FileRef,
        source: AcceptSource,
        elided: bool,
        evidence: Option<LevelEvidence>,
        round: Option<u32>,
    },
    /// An imported rule (tables_imported / evidence_imported provenance), from `open` only; its level is capped by
    /// the level the import record claims.
    Import {
        rule: String,
        level: EvidenceLevel,
        record: FileRef,
        provenance: String,
    },
    /// Higher evidence for the accepted rule (identity test / proof; also the plan's `relift_done`).
    Upgrade {
        level: EvidenceLevel,
        evidence: LevelEvidence,
    },
    /// Also the plan's `rule_removed` (from accepted, with the disproof artefact).
    Reject {
        reason: String,
        disproof: Option<FileRef>,
    },
    Escalate {
        to: Rung,
        reason: String,
    },
    Requeue {
        reason: String,
    },
    /// `exhausted` = the record of the item's ladder exhausted within budgets (required unless `preferred`).
    ProposeTerminal {
        reason: String,
        preferred: bool,
        exhausted: Option<FileRef>,
    },
    /// `count` >= 1 keys become terminals; `keys` = the key-set file. Non-preferred terminals count against the
    /// node's bound; preferred ones only if the spec says so.
    DeclareTerminal {
        count: u32,
        keys: FileRef,
        reason: String,
        preferred: bool,
    },
    MarkStale {
        input: String,
    },
    Reopen {
        reason: String,
    },
    /// Retire the item's accepted rule (economy, R31 / R33). Never deletion.
    Retire {
        rule: String,
        proof: RetirementProof,
        superseded_by: Option<String>,
    },
    /// Back from `retired` to `accepted` (its old level), e.g. after its replacement was rejected.
    Reinstate {
        reason: String,
    },
    /// Optimise mode (U-13): this accepted item replaces `replaces` (same node): requires a strictly better
    /// economy (Pareto) and the containment proof artefact (domain of the new rule contains the replaced ones).
    Replace {
        replaces: Vec<String>,
        containment: FileRef,
    },
    /// The plan's `key_excluded`: keys removed from the rule's credited domain (disproof artefact).
    ExcludeKeys {
        keys: FileRef,
        disproof: FileRef,
        reason: String,
    },
    /// The item needs a person (reason code + text); left only by an operator retry / skip or a reopen.
    Attention {
        code: String,
        text: String,
    },
}

/// The reason a rule may be retired, each naming its proof artefact by digest. On a FULL node only the
/// coverage-preserving kinds are legal (R33); `box_closure_pass` is legal only on a node whose target is its spec
/// box.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RetirementProof {
    EmptyDomain { proof: FileRef },
    FullyShadowed { proof: FileRef },
    ReplacedBy { proof: FileRef },
    DispatchOrderChange { proof: FileRef },
    Regeneration { table: FileRef, proof: FileRef },
    BoxClosurePass { closure: FileRef },
}

impl RetirementProof {
    pub fn box_only(&self) -> bool {
        matches!(self, RetirementProof::BoxClosurePass { .. })
    }
    pub fn word(&self) -> &'static str {
        match self {
            RetirementProof::EmptyDomain { .. } => "empty_domain",
            RetirementProof::FullyShadowed { .. } => "fully_shadowed",
            RetirementProof::ReplacedBy { .. } => "replaced_by",
            RetirementProof::DispatchOrderChange { .. } => "dispatch_order_change",
            RetirementProof::Regeneration { .. } => "regeneration",
            RetirementProof::BoxClosurePass { .. } => "box_closure_pass",
        }
    }
}

/// Economy measures of one accepted rule record (R31 (a)); `coefficient_degree` is a maximum, the rest add up.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleEconomy {
    pub cases: u64,
    pub rules: u64,
    pub rhs_terms: u64,
    pub monomials: u64,
    pub coefficient_degree: u64,
    /// Bytes in the shipped (canonical table) format.
    pub bytes: u64,
}

impl RuleEconomy {
    pub fn add(&mut self, o: &RuleEconomy) {
        self.cases += o.cases;
        self.rules += o.rules;
        self.rhs_terms += o.rhs_terms;
        self.monomials += o.monomials;
        self.coefficient_degree = self.coefficient_degree.max(o.coefficient_degree);
        self.bytes += o.bytes;
    }
    /// Pareto: no measure worse and at least one strictly smaller (U-13 "strictly better").
    pub fn strictly_better_than(&self, o: &RuleEconomy) -> bool {
        let a = [
            self.cases,
            self.rules,
            self.rhs_terms,
            self.monomials,
            self.coefficient_degree,
            self.bytes,
        ];
        let b = [
            o.cases,
            o.rules,
            o.rhs_terms,
            o.monomials,
            o.coefficient_degree,
            o.bytes,
        ];
        a.iter().zip(&b).all(|(x, y)| x <= y) && a.iter().zip(&b).any(|(x, y)| x < y)
    }
}

/// OUTCOME binding carried into `job.finished` (contract: OUTCOME bound to spec digest + attempt).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutcomeBinding {
    pub job: String,
    pub attempt: u32,
    pub spec_blake3: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum JobEvent {
    /// WRITE-AHEAD intent: fsynced before the child is spawned. `spec_blake3` = BLAKE3 of the attempt's `job.json`
    /// exactly as written (the digest the contract binds OUTCOME / RESUME to; the job-level `job_queued.spec_blake3`
    /// is the attempt-independent request digest).
    Launch {
        run_dir: String,
        cpus: Vec<u32>,
        reserved_bytes: u64,
        spec_blake3: String,
        /// Lane int5 (gate3 M5): the deterministic systemd unit name of the job's scope (`launch::scope_unit_name` of
        /// the campaign id and the attempt label), when the supervisor asked for one. A restart stops only a scope
        /// whose name is recorded here AND whose leader pid is recorded by `started` in this campaign's ledger.
        /// Omitted when absent (older records re-serialise unchanged).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        scope_unit: Option<String>,
    },
    Started {
        pid: u32,
        start_ticks: u64,
        boot_id: String,
    },
    /// `class` and `accept` = the contract's decision-table row (`classify`): `accept` must be one the row of
    /// `class` allows; records may be produced only when it is `canonical` / `committed_rules`, which needs the
    /// sealed table and the OUTCOME binding.
    Finished {
        class: FinishClass,
        accept: ContractAccept,
        exit_code: Option<i32>,
        signal: Option<i32>,
        sealed_output: Option<FileRef>,
        outcome: Option<FileRef>,
        binding: Option<OutcomeBinding>,
        cpu_ms: u64,
        wall_ms: u64,
        peak_rss_bytes: u64,
        time_decided: bool,
    },
    Interrupt {
        reason: String,
    },
    Lose {
        reason: String,
    },
    Requeue {
        reason: String,
    },
    Retry {
        reason: String,
    },
    Cancel {
        reason: String,
    },
    /// RECOVERY HOOK (unconfirmed launch): the attempt's launch intent was journalled in an earlier session and its
    /// start was never confirmed: EXECUTION UNKNOWN (state `unconfirmed`). Appended by a resuming supervisor for
    /// every attempt in [`crate::state::CampaignState::unconfirmed_launches`] before anything else is decided.
    Unconfirmed {
        reason: String,
    },
    /// Lane int5 (unconfirmed launch, the ONE proven "did not run"): the spawn of this unconfirmed launch provably
    /// never happened: its run directory is AUTHENTICATED (layout path, `RUN/sup/CAMPAIGN` = this campaign's marker id,
    /// job.json digest = the launch record's) and holds no spawn sentinel `RUN/sup/SPAWN` (written and fsynced right
    /// before every spawn), and no marked process of the attempt exists. `unconfirmed` -> `lost` (then `requeue`).
    /// `reason` names that evidence. Anything less is execution unknown (no automatic retry).
    NotSpawned {
        reason: String,
    },
}

/// Campaign mode (U-13): `close` (default) or `optimise` (boot from imported results; replacements only through
/// `item.replace`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CampaignMode {
    #[default]
    Close,
    Optimise,
}

/// Node spec values recorded in its identity record (U-6, U-7, U-12, R41): all optional, defaults in
/// [`crate::state::NodeSpec::resolve`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NodeSpecValues {
    /// Certification level of a FULL closure (default `replay`).
    pub certify_full: Option<CertLevel>,
    /// Certification level of a BOXED closure (default `per-key`).
    pub certify_boxed: Option<CertLevel>,
    /// Full-closure attempt budget before the box fallback (default 50 CPU-hours).
    pub full_attempt_cpu_hours: Option<u32>,
    /// ... or this many fixer rounds (default 5), whichever first.
    pub full_attempt_fixer_rounds: Option<u32>,
    /// Lowest rung at which a non-preferred terminal may be proposed (default L5).
    pub terminal_rung: Option<Rung>,
    /// The user's preferred-terminal list (R41 P-4): a preference, never a restriction.
    pub preferred_terminals: Option<FileRef>,
    /// Number of keys in that list (for the "preferred not needed" count), if known.
    pub preferred_listed: Option<u64>,
    /// Whether preferred terminals count against the node's terminal bound (default false).
    pub preferred_count_against_bound: Option<bool>,
    /// lane closeloop3 (closeloop2's request; closeloop.md 13.3 / 16.5): the target the node STARTS on (None = full,
    /// as before; written only when given, so older records are unchanged). `boxed` needs the spec box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_target: Option<StartTarget>,
}

/// The target a node starts on (lane closeloop3): `full` (default) or `boxed`: BOXED from its first record (the
/// spec box; a node whose spec gives the box as the user's fallback and which never makes a FULL attempt). Such a node
/// never had a FULL target: a reopen returns it to its box (R33 unchanged: a node closed FULL once is never boxed).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartTarget {
    #[default]
    Full,
    Boxed,
}

impl StartTarget {
    /// `full` (the default: not serialized in a resolved node spec, so older snapshots stay byte-identical).
    pub fn is_full(&self) -> bool {
        *self == StartTarget::Full
    }
}

/// What an operator control does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlVerb {
    Pause,
    Resume,
    Stop,
    Retry,
    Skip,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "scope", rename_all = "snake_case")]
pub enum ControlTarget {
    Campaign,
    Node { node: String },
    Item { item: String },
}

/// Operator control life cycle (plan 3.3.8 / 3.3.2 `control_applied` / `control_refused`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum ControlEvent {
    Requested {
        verb: ControlVerb,
        target: ControlTarget,
        by: String,
        reason: String,
    },
    Applied {
        effect: String,
    },
    Refused {
        why: String,
    },
}

/// Envelope of the hand-down ranges a closure was computed against (R41 P-2): `measured` = provisional.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvelopeKind {
    Measured,
    Proved,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum ExportEvent {
    Candidate { manifest: FileRef },
    Published { manifest: FileRef },
    Revoked { cause: String },
}

/// The closed list of record kinds of `rustred.campaign.ledger.v1` (additions = minor version, readers of an
/// older minor refuse unknown kinds: the ledger is never read tolerantly).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "body", rename_all = "snake_case")]
pub enum Event {
    CampaignCreated {
        campaign_id: String,
        spec_blake3: String,
        terminal_bound: u32,
        mode: CampaignMode,
    },
    Campaign {
        ev: CampaignEvent,
        reason: String,
    },
    SupervisorStarted {
        identity: ProcessIdentity,
        argv: Vec<String>,
        version: String,
        lock_token: String,
    },
    LockTakeover {
        previous: OwnerRecord,
        reason: DeadReason,
        orphans: Vec<u32>,
    },
    /// Torn-tail recovery or (`forced`) an operator truncation override.
    Recovery {
        segment: u32,
        last_good_seq: u64,
        truncated_bytes: u64,
        saved_as: Option<String>,
        detail: String,
        forced: bool,
    },
    SegmentOpened {
        index: u32,
        previous_blake3: String,
        snapshot_seq: u64,
    },
    InputRegistered {
        input: String,
        role: String,
        path: String,
        bytes: u64,
        blake3: String,
    },
    /// Hand-made evidence with provenance (M4); `level_claimed` caps what may be accepted from it.
    /// `producer_blake3` = digest of the producing binary; `None` (historical material whose producer was never
    /// recorded) requires a `provenance_class` (one of [`PROVENANCE_CLASSES`]) and makes the evidence PROVENANCE ONLY:
    /// it can never by itself give an item an evidence level (accept / import from it are refused; a check of our own
    /// is needed).
    EvidenceImported {
        evidence: String,
        producer: String,
        description: String,
        files: Vec<FileRef>,
        level_claimed: EvidenceLevel,
        producer_blake3: Option<String>,
        provenance_class: Option<String>,
    },
    /// An imported table set / earlier campaign result (U-13, R41 P-1 import first): provenance with digests;
    /// `level_claimed` caps the level of every rule imported from it. The source is never modified.
    TablesImported {
        provenance: String,
        producer: String,
        source_campaign: Option<String>,
        manifest: FileRef,
        files: Vec<FileRef>,
        level_claimed: EvidenceLevel,
    },
    /// `spec_box` = the input id (role `box`) of the user-provided box of the node's spec, if any (R35, U-7).
    /// `reference` = comparison-only numbers (old ledger, knock-out flag, master count: U-9); never read by any
    /// check.
    NodeRegistered {
        node: String,
        inputs: Vec<String>,
        terminal_bound: Option<u32>,
        spec_box: Option<String>,
        spec: NodeSpecValues,
        reference: Option<BTreeMap<String, String>>,
    },
    /// Economy measures of the item's accepted rule (re-measuring replaces the previous values).
    RuleEconomy {
        item: String,
        economy: RuleEconomy,
    },
    Node {
        node: String,
        ev: NodeEvent,
    },
    ItemCreated {
        item: String,
        node: String,
        kind: String,
        rung: Rung,
        parent: Option<String>,
        inputs: Vec<String>,
        label: String,
        weight: u64,
    },
    Item {
        item: String,
        ev: ItemEvent,
    },
    /// `inputs` = further input ids the job depends on (seed policy, tables, ...); the reducer stamps the job with
    /// the digests of these, the binary, the item's and the node's inputs (input identity, audit A C1).
    JobQueued {
        job: String,
        item: String,
        rung: Rung,
        kind: String,
        spec_blake3: String,
        binary: String,
        reserved_bytes: u64,
        inputs: Vec<String>,
    },
    Job {
        job: String,
        attempt: u32,
        ev: JobEvent,
    },
    /// Write-ahead intent of a side effect (`action` e.g. `write_accepted_record`, `write_keyset`, `kill_orphan`).
    Intent {
        op: String,
        action: String,
        target: String,
    },
    Done {
        op: String,
        result: Option<FileRef>,
    },
    Control {
        id: String,
        ev: ControlEvent,
    },
    OrphanKilled {
        job: Option<String>,
        pid: u32,
        start_ticks: u64,
    },
    /// Lane int5 (gate3 M2): ONE record per signal the supervisor's process layer DELIVERED (through the guard, the
    /// one signal path, or a cgroup kill): process / group / cgroup, the start time the identity checklist read, the
    /// marker it read (campaign id and attempt label), the proof of belonging, and who signalled why (`by`).
    /// Informational (no state change); written by the supervisor after the delivery (group commit per tick).
    SignalSent {
        job: Option<String>,
        pid: u32,
        start_ticks: Option<u64>,
        signal: i32,
        target: String,
        marker: Option<String>,
        proof: String,
        by: String,
    },
    VerifyCheckpoint {
        seq: u64,
        clean: bool,
        report: Option<FileRef>,
    },
    /// An artefact named by a committed record is missing / altered: the named items become stale.
    ArtefactLost {
        path: String,
        blake3: String,
        items: Vec<String>,
    },
    Export {
        rev: String,
        ev: ExportEvent,
    },
    Note {
        text: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Durability {
    Sync,
    Lazy,
}

impl Event {
    pub fn kind(&self) -> &'static str {
        match self {
            Event::CampaignCreated { .. } => "campaign_created",
            Event::Campaign { .. } => "campaign",
            Event::SupervisorStarted { .. } => "supervisor_started",
            Event::LockTakeover { .. } => "lock_takeover",
            Event::Recovery { .. } => "recovery",
            Event::SegmentOpened { .. } => "segment_opened",
            Event::InputRegistered { .. } => "input_registered",
            Event::EvidenceImported { .. } => "evidence_imported",
            Event::TablesImported { .. } => "tables_imported",
            Event::NodeRegistered { .. } => "node_registered",
            Event::RuleEconomy { .. } => "rule_economy",
            Event::Node { .. } => "node",
            Event::ItemCreated { .. } => "item_created",
            Event::Item { .. } => "item",
            Event::JobQueued { .. } => "job_queued",
            Event::Job { .. } => "job",
            Event::Intent { .. } => "intent",
            Event::Done { .. } => "done",
            Event::Control { .. } => "control",
            Event::OrphanKilled { .. } => "orphan_killed",
            Event::SignalSent { .. } => "signal_sent",
            Event::VerifyCheckpoint { .. } => "verify_checkpoint",
            Event::ArtefactLost { .. } => "artefact_lost",
            Event::Export { .. } => "export",
            Event::Note { .. } => "note",
        }
    }

    /// The fsync class (module doc).
    pub fn durability(&self) -> Durability {
        match self {
            Event::ItemCreated { .. }
            | Event::JobQueued { .. }
            | Event::Item {
                ev: ItemEvent::Start { .. },
                ..
            }
            | Event::Note { .. } => Durability::Lazy,
            _ => Durability::Sync,
        }
    }
}

/// One ledger record (the line adds the `h` suffix, see the module doc).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub seq: u64,
    pub utc: String,
    pub prev: String,
    pub event: Event,
}

/// blake3 hex of bytes.
pub fn line_hash(line: &[u8]) -> String {
    blake3::hash(line).to_hex().to_string()
}

/// Encodes a record into its line (without newline) and returns `(line, h)`.
pub fn encode_record(rec: &Record) -> (Vec<u8>, String) {
    let body = serde_json::to_vec(rec).expect("records serialise");
    let h = line_hash(&body);
    let mut line = Vec::with_capacity(body.len() + SUFFIX_LEN);
    line.extend_from_slice(&body[..body.len() - 1]);
    line.extend_from_slice(b",\"h\":\"");
    line.extend_from_slice(h.as_bytes());
    line.extend_from_slice(b"\"}");
    (line, h)
}

/// Decodes one line (without newline): framing, digest, canonical form. Returns `(record, h)`.
pub fn decode_record(line: &[u8]) -> Result<(Record, String), String> {
    if line.len() <= SUFFIX_LEN + 2 {
        return Err("line too short for a record".into());
    }
    let split = line.len() - SUFFIX_LEN;
    let suffix = &line[split..];
    if !suffix.starts_with(b",\"h\":\"") || !suffix.ends_with(b"\"}") {
        return Err("no record digest suffix".into());
    }
    let h = &suffix[6..70];
    if !h.iter().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f')) {
        return Err("record digest is not lowercase hex".into());
    }
    let mut body = Vec::with_capacity(split + 1);
    body.extend_from_slice(&line[..split]);
    body.push(b'}');
    let h = std::str::from_utf8(h).expect("hex is ascii").to_string();
    if line_hash(&body) != h {
        return Err("record digest mismatch (line altered)".into());
    }
    let rec: Record =
        serde_json::from_slice(&body).map_err(|e| format!("digest ok but unparsable: {e}"))?;
    if serde_json::to_vec(&rec).expect("records serialise") != body {
        return Err("record is not in canonical form".into());
    }
    if rec.schema != LEDGER_SCHEMA {
        return Err(format!("unknown schema {:?}", rec.schema));
    }
    Ok((rec, h))
}

// ------------------------------------------------------------------------------------------------------------
// Manifest, snapshot, commit mark
// ------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedSegment {
    pub index: u32,
    pub file: String,
    pub first_seq: u64,
    pub last_seq: u64,
    pub bytes: u64,
    pub blake3: String,
    /// Hash (`h`) of its last record (= `prev` of the next segment's first record).
    pub last_hash: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotRef {
    pub file: String,
    pub seq: u64,
    pub blake3: String,
    pub state_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub schema: String,
    pub campaign_id: String,
    pub sealed: Vec<SealedSegment>,
    pub active: ActiveSegment,
    pub snapshot: Option<SnapshotRef>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveSegment {
    pub index: u32,
    pub file: String,
    pub first_seq: u64,
    pub prev: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema: String,
    pub seq: u64,
    pub head: String,
    pub state: CampaignState,
}

/// The durable commit mark: everything up to `offset` of segment `segment` (record `seq`, chain value `head`)
/// was fsynced before an acknowledgement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitMark {
    pub counter: u64,
    pub segment: u32,
    pub seq: u64,
    pub offset: u64,
    pub head: String,
}

impl CommitMark {
    fn encode(&self) -> Vec<u8> {
        let text = format!(
            "RRCM1 {} {} {} {} {}",
            self.counter, self.segment, self.seq, self.offset, self.head
        );
        let c = &line_hash(text.as_bytes())[..16];
        let mut s = format!("{text} {c}");
        while s.len() < MARK_SLOT_BYTES - 1 {
            s.push(' ');
        }
        s.push('\n');
        s.into_bytes()
    }
    fn decode(slot: &[u8]) -> Option<CommitMark> {
        let s = std::str::from_utf8(slot).ok()?.trim_end();
        let (text, c) = s.rsplit_once(' ')?;
        if &line_hash(text.as_bytes())[..16] != c {
            return None;
        }
        let f: Vec<&str> = text.split(' ').collect();
        if f.len() != 6 || f[0] != "RRCM1" {
            return None;
        }
        Some(CommitMark {
            counter: f[1].parse().ok()?,
            segment: f[2].parse().ok()?,
            seq: f[3].parse().ok()?,
            offset: f[4].parse().ok()?,
            head: f[5].to_string(),
        })
    }
    /// The valid slot with the highest counter, if any.
    pub fn read(bytes: &[u8]) -> Option<CommitMark> {
        bytes
            .chunks(MARK_SLOT_BYTES)
            .take(2)
            .filter_map(CommitMark::decode)
            .max_by_key(|m| m.counter)
    }
}

/// Reads the commit mark of a ledger directory. A ledger always has a valid slot (record 1 is fsynced and marked
/// before the manifest exists), so a missing file or a file without a valid slot is corruption.
pub fn read_commit_mark(dir: &Path) -> Result<Option<CommitMark>, LedgerError> {
    let p = dir.join(COMMIT_FILE);
    match fs::read(&p) {
        Ok(b) => match CommitMark::read(&b) {
            Some(m) => Ok(Some(m)),
            None => Err(LedgerError::Corrupt {
                file: COMMIT_FILE.into(),
                line: 0,
                why: "commit mark has no valid slot".into(),
            }),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(LedgerError::Corrupt {
            file: COMMIT_FILE.into(),
            line: 0,
            why: "commit mark file missing".into(),
        }),
        Err(e) => Err(io_err(&p, "read commit mark")(e)),
    }
}

fn segment_name(index: u32) -> String {
    format!("segment-{index:06}.jsonl")
}

fn snapshot_name(seq: u64) -> String {
    format!("snapshot-{seq:012}.json")
}

// ------------------------------------------------------------------------------------------------------------
// File operations seam (fsync-policy tests, fault injection)
// ------------------------------------------------------------------------------------------------------------

/// An open file of the ledger (segment or commit mark). `append` writes at the tracked end with one positioned
/// write; `truncate` = set_len + fsync; `write_at` rewrites bytes in place (commit mark, fsync gate).
pub trait SegmentFile: Send {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
    fn truncate(&mut self, len: u64) -> io::Result<()>;
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()>;
}

/// Every file operation of the ledger goes through this trait. Default methods do the real thing, so a test
/// fake overrides only what it records or breaks.
pub trait FileOps: Send + Sync {
    /// Opens (creating) a segment for appending at its current end.
    fn open_segment(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        RealFile::open(path)
    }
    /// Opens (creating) the commit-mark file.
    fn open_mark(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        RealFile::open(path)
    }
    /// Creates `path` (O_EXCL), writes all bytes, fsyncs it.
    fn write_new_synced(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut f = FsOpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        f.write_all(bytes)?;
        f.sync_all()
    }
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }
    fn sync_dir(&self, dir: &Path) -> io::Result<()> {
        atomic::sync_dir(dir)
    }
    fn remove(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }
    /// Creates `path` (O_EXCL), streams its content through `fill` (buffered), fsyncs it: the BACKGROUND
    /// snapshot writer (a large state is never held as one byte buffer).
    fn write_new_synced_with(
        &self,
        path: &Path,
        fill: &mut dyn FnMut(&mut dyn Write) -> io::Result<()>,
    ) -> io::Result<()> {
        let f = FsOpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        let mut w = io::BufWriter::with_capacity(1 << 20, f);
        fill(&mut w)?;
        let f = w.into_inner().map_err(|e| e.into_error())?;
        f.sync_all()
    }
}

/// The real file operations.
#[derive(Clone, Copy, Debug, Default)]
pub struct RealOps;
impl FileOps for RealOps {}

/// The real segment / mark file (no O_APPEND, so positioned rewrites work; the ledger is the only writer).
pub struct RealFile {
    file: File,
    len: u64,
}

impl RealFile {
    pub fn open(path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        let file = FsOpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;
        let len = file.metadata()?.len();
        Ok(Box::new(RealFile { file, len }))
    }
}

impl SegmentFile for RealFile {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all_at(bytes, self.len)?;
        self.len += bytes.len() as u64;
        Ok(())
    }
    fn sync(&mut self) -> io::Result<()> {
        self.file.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.file.set_len(len)?;
        self.len = len;
        self.file.sync_all()
    }
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        self.file.write_all_at(bytes, offset)?;
        self.len = self.len.max(offset + bytes.len() as u64);
        Ok(())
    }
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Where an atomic install failed: before the rename (target untouched) or after it (target may be new).
#[derive(Debug)]
enum InstallFail {
    BeforeRename(io::Error),
    AfterRename(io::Error),
}

/// temp (O_EXCL) + write + fsync, rename over `path`, directory fsync.
fn install(ops: &dyn FileOps, path: &Path, bytes: &[u8]) -> Result<(), InstallFail> {
    let dir = path.parent().unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(".{name}.rr-tmp-{}-{n}", std::process::id()));
    if let Err(e) = ops.write_new_synced(&temp, bytes) {
        let _ = fs::remove_file(&temp);
        return Err(InstallFail::BeforeRename(e));
    }
    if let Err(e) = ops.rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(InstallFail::BeforeRename(e));
    }
    ops.sync_dir(dir).map_err(InstallFail::AfterRename)
}

fn install_err(path: &Path, f: InstallFail) -> LedgerError {
    match f {
        InstallFail::BeforeRename(e) | InstallFail::AfterRename(e) => io_err(path, "install")(e),
    }
}

// ------------------------------------------------------------------------------------------------------------
// Errors and configuration
// ------------------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum LedgerError {
    /// ENOSPC / EDQUOT: the failed batch was truncated away (best effort); the handle is poisoned (reopen after
    /// freeing space).
    NoSpace {
        path: PathBuf,
    },
    Io {
        path: PathBuf,
        op: &'static str,
        source: io::Error,
    },
    Atomic(AtomicError),
    /// The record was refused by the reducer and NOT written.
    Refused(ReduceError),
    /// A stored record is refused by the reducer (code/ledger mismatch or tampering).
    Replay(ReduceError),
    /// Framing / digest / canonical form / chain / sequence / commit-mark violation that is not a torn tail.
    Corrupt {
        file: String,
        line: u64,
        why: String,
    },
    NotALedger {
        dir: PathBuf,
    },
    AlreadyExists {
        dir: PathBuf,
    },
    TooLarge {
        bytes: usize,
    },
    /// The lock guard passed to a write handle belongs to another directory.
    WrongLock {
        lock_dir: PathBuf,
        cdir: PathBuf,
    },
    /// Another write handle holds the ledger directory.
    Busy {
        dir: PathBuf,
    },
    /// A previous write or fsync failed; drop the handle and `open` again.
    Poisoned {
        why: String,
    },
    /// `ledger/` holds an acknowledged history but no MANIFEST.json (audit-ledger2 M1): a DAMAGED ledger. `create`
    /// refuses (it never deletes such files); a supervisor stops. Recovery: [`rebuild_manifest`] (verified chain ->
    /// manifest) after a person has looked at the directory.
    ManifestMissing {
        dir: PathBuf,
        found: String,
    },
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::NoSpace { path } => write!(
                f,
                "disk full appending to {}; reopen after freeing space",
                path.display()
            ),
            LedgerError::Io { path, op, source } => {
                write!(f, "ledger I/O ({op}) on {}: {source}", path.display())
            }
            LedgerError::Atomic(e) => write!(f, "{e}"),
            LedgerError::Refused(e) => write!(f, "record refused (not written): {e}"),
            LedgerError::Replay(e) => write!(f, "stored record refused on replay: {e}"),
            LedgerError::Corrupt { file, line, why } => {
                write!(f, "ledger corrupt at {file}:{line}: {why}")
            }
            LedgerError::NotALedger { dir } => write!(f, "{} holds no ledger", dir.display()),
            LedgerError::AlreadyExists { dir } => {
                write!(f, "{} already holds a ledger", dir.display())
            }
            LedgerError::TooLarge { bytes } => {
                write!(f, "record of {bytes} bytes exceeds {MAX_RECORD_BYTES}")
            }
            LedgerError::WrongLock { lock_dir, cdir } => write!(
                f,
                "lock of {} cannot open the ledger of {}",
                lock_dir.display(),
                cdir.display()
            ),
            LedgerError::Busy { dir } => {
                write!(f, "another write handle holds {}", dir.display())
            }
            LedgerError::Poisoned { why } => {
                write!(f, "ledger handle poisoned ({why}); drop it and reopen")
            }
            LedgerError::ManifestMissing { dir, found } => write!(
                f,
                "damaged ledger: {} has no {MANIFEST_FILE} but holds files ({found}); nothing was deleted (never a repair by deletion); if it holds a history (also the record 1 of a create that crashed), rebuild the manifest from the verified chain (`rr_campaign rebuild-manifest CDIR`); otherwise check it and remove it by hand",
                dir.display()
            ),
        }
    }
}

impl std::error::Error for LedgerError {}

impl From<AtomicError> for LedgerError {
    fn from(e: AtomicError) -> Self {
        match e {
            AtomicError::NoSpace { path, .. } => LedgerError::NoSpace { path },
            e => LedgerError::Atomic(e),
        }
    }
}

fn io_err<'a>(path: &'a Path, op: &'static str) -> impl FnOnce(io::Error) -> LedgerError + 'a {
    move |e| {
        if atomic::is_no_space(&e) {
            LedgerError::NoSpace {
                path: path.to_path_buf(),
            }
        } else {
            LedgerError::Io {
                path: path.to_path_buf(),
                op,
                source: e,
            }
        }
    }
}

fn corrupt(file: &str, line: u64, why: impl Into<String>) -> LedgerError {
    LedgerError::Corrupt {
        file: file.into(),
        line,
        why: why.into(),
    }
}

#[derive(Clone, Debug)]
pub struct LedgerConfig {
    /// Rotate the active segment once it reaches this size (checked after each append).
    pub rotate_bytes: u64,
    pub lazy_max_records: usize,
    pub lazy_max_ms: u64,
    /// Snapshots kept besides the one in the manifest.
    pub keep_old_snapshots: usize,
    /// Minimum monotonic time between two attempts of a failed rotation.
    pub rotation_retry_ms: u64,
    /// A rotation writes the snapshot INLINE while the state has fewer items + jobs than this; from this size on
    /// the state is frozen (copy-on-write maps: milliseconds) and the snapshot is streamed to disk by a background
    /// thread, referenced from the next rotation on, so appends never wait for it (audit-ledger2 m10).
    pub background_snapshot_min_entries: usize,
    /// Background mode: a rotation WAITS for the running snapshot only when the manifest's snapshot already lags
    /// this many sealed segments behind (appends outpacing snapshots; never at campaign rates), so `open` never
    /// replays more than about this many segments after the snapshot.
    pub snapshot_max_lag_segments: usize,
}

impl Default for LedgerConfig {
    fn default() -> Self {
        LedgerConfig {
            rotate_bytes: 64 << 20,
            lazy_max_records: 256,
            lazy_max_ms: 1000,
            keep_old_snapshots: 1,
            rotation_retry_ms: 10_000,
            background_snapshot_min_entries: 20_000,
            snapshot_max_lag_segments: 16,
        }
    }
}

/// Options of [`Ledger::open_with`].
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    /// Operator override (`--accept-truncation-to <seq>`): when the ACTIVE segment is corrupt after record `seq`
    /// (or the commit mark names records that are gone), truncate it after record `seq`, save the dropped bytes
    /// and append a `recovery` record with `forced: true`. Never applies to sealed segments.
    pub accept_truncation_to: Option<u64>,
}

/// Provenance classes of evidence whose producer digest is unknown (`evidence_imported.provenance_class`; the accept
/// schema's `rule.producer.provenance`): `recorded` only beside a digest.
pub const PROVENANCE_CLASSES: [&str; 3] = [
    "recorded",
    "unknown: historical import",
    "unknown: not recorded by the producer",
];

/// Values of record 1.
#[derive(Clone, Debug)]
pub struct Genesis {
    pub campaign_id: String,
    pub spec_blake3: String,
    pub terminal_bound: u32,
    pub mode: CampaignMode,
}

impl Genesis {
    pub fn new(campaign_id: &str, spec_blake3: &str, terminal_bound: u32) -> Self {
        Genesis {
            campaign_id: campaign_id.into(),
            spec_blake3: spec_blake3.into(),
            terminal_bound,
            mode: CampaignMode::Close,
        }
    }
}

/// What `open` did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenReport {
    pub from_snapshot: Option<u64>,
    pub replayed: u64,
    pub recovery: Option<RecoveryInfo>,
    pub removed_unreferenced: Vec<String>,
    /// Bytes beyond the commit mark rewritten and fsynced (fsync gate).
    pub regated_bytes: u64,
    /// Why the manifest's snapshot was not used (missing, older snapshot version: full replay), if so.
    pub snapshot_note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryInfo {
    pub segment: u32,
    pub last_good_seq: u64,
    pub truncated_bytes: u64,
    pub saved_as: Option<String>,
    pub detail: String,
    pub forced: bool,
}

// ------------------------------------------------------------------------------------------------------------
// Reading: base state, segment scan
// ------------------------------------------------------------------------------------------------------------

/// Path of the ledger directory inside a campaign directory.
pub fn ledger_dir(cdir: &Path) -> PathBuf {
    cdir.join(LEDGER_DIR)
}

/// Captures the state and head right after record `at` during a scan (streaming `verify`, audit B M4).
pub struct Capture {
    pub at: u64,
    pub got: Option<(CampaignState, String)>,
}

/// What a scan does with every verified record, in order (reduce it, capture the state, collect it, ...).
type Visit<'v> = dyn FnMut(&Record, &str) -> Result<(), LedgerError> + 'v;

/// The visitor that reduces into `st` (a refusal is `Replay`) and fills `cap` right after its record.
fn reducer<'a>(
    st: &'a mut CampaignState,
    mut cap: Option<&'a mut Capture>,
) -> impl FnMut(&Record, &str) -> Result<(), LedgerError> + 'a {
    move |rec, h| {
        st.apply_record(rec).map_err(LedgerError::Replay)?;
        if let Some(c) = cap.as_deref_mut()
            && c.at == rec.seq
        {
            c.got = Some((st.clone(), h.to_string()));
        }
        Ok(())
    }
}

/// Reads a segment named by the manifest. A MISSING segment is corruption (audit-ledger2 m8), not an I/O error:
/// segments are never deleted.
fn read_segment_file(dir: &Path, file: &str) -> Result<Vec<u8>, LedgerError> {
    let p = dir.join(file);
    match fs::read(&p) {
        Ok(b) => Ok(b),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Err(corrupt(
            file,
            0,
            "segment named by the manifest is missing (deleted)",
        )),
        Err(e) => Err(io_err(&p, "read segment")(e)),
    }
}

/// Result of scanning the active segment.
struct Scan {
    replayed: u64,
    /// End of the last good record.
    pos: usize,
    seq: u64,
    head: String,
    hasher: blake3::Hasher,
    /// A torn tail (beyond the mark, an incomplete line or NUL-filled bytes).
    torn: Option<String>,
    /// Corruption found (only returned when an override may handle it; else an error).
    corrupt: Option<(u64, String)>,
}

fn is_torn_shape(line_incomplete: bool, bytes: &[u8]) -> bool {
    line_incomplete || bytes.contains(&0)
}

/// Scans `bytes` (the active segment) after record `seq` / chain value `head`, handing every verified record to
/// `visit`. `mark` = the commit mark if it names this segment; `stop_after` = stop after this seq (override).
/// Classification of the first bad line at offset p (module doc): torn only if beyond the mark and torn-shaped
/// (incomplete or containing NUL bytes). Since a failed commit-mark write poisons the handle (audit-ledger2 m7),
/// no acknowledged record ever lies beyond the mark, so a torn-shaped region there is cut even when complete valid
/// (unacknowledged) records follow it (out-of-order write-back of unsynced pages after a power loss).
#[allow(clippy::too_many_arguments)]
fn scan_active(
    file: &str,
    bytes: &[u8],
    mut seq: u64,
    mut head: String,
    mark: Option<&CommitMark>,
    stop_after: Option<u64>,
    visit: &mut Visit<'_>,
) -> Result<Scan, LedgerError> {
    let mut pos = 0usize;
    let mut line_no = 0u64;
    let mut hasher = blake3::Hasher::new();
    let mut replayed = 0u64;
    let mut fail: Option<(String, bool)> = None; // (why, torn-shaped)
    let mut mark_hit = mark.is_none() || mark.is_some_and(|m| m.offset == 0);
    while pos < bytes.len() {
        if stop_after.is_some_and(|s| seq >= s) {
            break;
        }
        line_no += 1;
        let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
            fail = Some((
                format!(
                    "line {line_no}: incomplete last line ({} bytes, no newline)",
                    bytes.len() - pos
                ),
                true,
            ));
            break;
        };
        let line = &bytes[pos..pos + nl];
        let (rec, h) = match decode_record(line) {
            Ok(x) => x,
            Err(why) => {
                fail = Some((format!("line {line_no}: {why}"), is_torn_shape(false, line)));
                break;
            }
        };
        if rec.seq != seq + 1 {
            fail = Some((
                format!("line {line_no}: seq {} after {seq}", rec.seq),
                false,
            ));
            break;
        }
        if rec.prev != head {
            fail = Some((
                format!(
                    "line {line_no}: hash chain broken (prev does not match the previous record)"
                ),
                false,
            ));
            break;
        }
        visit(&rec, &h)?;
        seq = rec.seq;
        head = h;
        hasher.update(&bytes[pos..pos + nl + 1]);
        replayed += 1;
        pos += nl + 1;
        if let Some(m) = mark
            && m.offset == pos as u64
        {
            if m.seq != seq || m.head != head {
                return Err(corrupt(
                    file,
                    line_no,
                    format!(
                        "commit mark at offset {} names seq {} / {}, the ledger has seq {seq} / {head} there",
                        m.offset, m.seq, m.head
                    ),
                ));
            }
            mark_hit = true;
        }
    }
    let mut scan = Scan {
        replayed,
        pos,
        seq,
        head,
        hasher,
        torn: None,
        corrupt: None,
    };
    if stop_after.is_some() {
        return Ok(scan);
    }
    let mark_off = mark.map(|m| m.offset).unwrap_or(0);
    if let Some((why, torn_shape)) = fail {
        let mut why = why;
        let is_torn = torn_shape && (pos as u64) >= mark_off && mark_hit;
        if !torn_shape {
            why.push_str(" (a complete line: corruption, not a torn write)");
        } else if (pos as u64) < mark_off || !mark_hit {
            why.push_str(&format!(
                " (at or before the commit mark at offset {mark_off}: acknowledged data damaged)"
            ));
        }
        if is_torn {
            scan.torn = Some(why);
        } else {
            scan.corrupt = Some((line_no, format!("{why}; last good record seq {seq}")));
        }
    } else if !mark_hit {
        scan.corrupt = Some((
            line_no,
            format!(
                "commit mark names seq {} at offset {mark_off}, but the segment ends at offset {pos} (seq {seq}): acknowledged records are missing",
                mark.map(|m| m.seq).unwrap_or(0)
            ),
        ));
    }
    Ok(scan)
}

/// Replays one sealed segment through `visit`, checking its digest, framing, sequence and chain.
fn replay_sealed(
    dir: &Path,
    s: &SealedSegment,
    mut seq: u64,
    head: &str,
    visit: &mut Visit<'_>,
) -> Result<(u64, String), LedgerError> {
    let bytes = read_segment_file(dir, &s.file)?;
    if bytes.len() as u64 != s.bytes || blake3::hash(&bytes).to_hex().as_str() != s.blake3 {
        return Err(corrupt(
            &s.file,
            0,
            "sealed segment size or digest differs from the manifest (edited or truncated)",
        ));
    }
    let mut head = head.to_string();
    let mut n = 0u64;
    let mut pos = 0usize;
    while pos < bytes.len() {
        let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
            return Err(corrupt(&s.file, n + 1, "sealed segment ends inside a line"));
        };
        let (rec, h) =
            decode_record(&bytes[pos..pos + nl]).map_err(|why| corrupt(&s.file, n + 1, why))?;
        if rec.seq != seq + 1 || rec.prev != head {
            return Err(corrupt(&s.file, n + 1, "sequence or hash chain broken"));
        }
        visit(&rec, &h)?;
        seq = rec.seq;
        head = h;
        n += 1;
        pos += nl + 1;
    }
    if seq != s.last_seq || head != s.last_hash {
        return Err(corrupt(
            &s.file,
            n,
            "sealed segment ends elsewhere than the manifest says",
        ));
    }
    Ok((n, head))
}

/// Checks a sealed segment against the manifest without replaying it: size, BLAKE3, and its place in the chain
/// (first record = `seq_before + 1` continuing `head_before`, last record = the manifest's `last_seq` / `last_hash`),
/// so `open` trusts no manifest field that the full readers would refuse. Returns its (last seq, last hash).
fn check_sealed(
    dir: &Path,
    s: &SealedSegment,
    seq_before: u64,
    head_before: &str,
) -> Result<(u64, String), LedgerError> {
    let bytes = read_segment_file(dir, &s.file)?;
    if bytes.len() as u64 != s.bytes {
        return Err(corrupt(
            &s.file,
            0,
            format!(
                "sealed segment has {} bytes, the manifest says {}",
                bytes.len(),
                s.bytes
            ),
        ));
    }
    if blake3::hash(&bytes).to_hex().as_str() != s.blake3 {
        return Err(corrupt(
            &s.file,
            0,
            "sealed segment digest differs from the manifest",
        ));
    }
    let bad = |why: &str| corrupt(&s.file, 0, format!("sealed segment {why}"));
    if s.first_seq != seq_before + 1 || !bytes.ends_with(b"\n") {
        return Err(bad(
            "does not continue the chain (first seq) or ends inside a line",
        ));
    }
    let first_nl = bytes
        .iter()
        .position(|&b| b == b'\n')
        .expect("ends with a newline");
    let (first, _) = decode_record(&bytes[..first_nl]).map_err(|w| bad(&w))?;
    if first.seq != seq_before + 1 || first.prev != head_before {
        return Err(bad("first record does not continue the chain"));
    }
    let body = &bytes[..bytes.len() - 1];
    let last_start = body
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let (last, h) = decode_record(&body[last_start..]).map_err(|w| bad(&w))?;
    if last.seq != s.last_seq || h != s.last_hash {
        return Err(bad("ends elsewhere than the manifest says"));
    }
    Ok((s.last_seq, s.last_hash.clone()))
}

// ------------------------------------------------------------------------------------------------------------
// Snapshots: streamed in both directions (audit-ledger2 m10)
// ------------------------------------------------------------------------------------------------------------

fn json_str(s: &str) -> String {
    serde_json::to_string(s).expect("strings serialise")
}

/// The bytes of a snapshot file before its state: `{"schema":..,"seq":N,"head":"..","state":` (the file is exactly
/// `serde_json` of [`Snapshot`] plus a newline: prefix, state, `}\n`).
fn snapshot_prefix(seq: u64, head: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":{},\"seq\":{seq},\"head\":{},\"state\":",
        json_str(SNAPSHOT_SCHEMA),
        json_str(head)
    )
    .into_bytes()
}

/// Writes through to `w`, feeding the file hasher and the state hasher.
struct Tee<'a> {
    w: &'a mut dyn Write,
    file: &'a mut blake3::Hasher,
    state: &'a mut blake3::Hasher,
}

impl Write for Tee<'_> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = self.w.write(b)?;
        self.file.update(&b[..n]);
        self.state.update(&b[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.w.flush()
    }
}

/// Serialises the snapshot of `state` (after record `seq`, chain value `head`) into `w`, streaming; returns (BLAKE3
/// of the file bytes, BLAKE3 of the state's JSON = [`CampaignState::digest`]).
fn write_snapshot_to(
    w: &mut dyn Write,
    seq: u64,
    head: &str,
    state: &CampaignState,
) -> io::Result<(String, String)> {
    let mut fh = blake3::Hasher::new();
    let mut sh = blake3::Hasher::new();
    let pre = snapshot_prefix(seq, head);
    w.write_all(&pre)?;
    fh.update(&pre);
    {
        let mut tee = Tee {
            w: &mut *w,
            file: &mut fh,
            state: &mut sh,
        };
        serde_json::to_writer(&mut tee, state).map_err(io::Error::other)?;
    }
    w.write_all(b"}\n")?;
    fh.update(b"}\n");
    Ok((
        fh.finalize().to_hex().to_string(),
        sh.finalize().to_hex().to_string(),
    ))
}

/// Reads through, feeding a hasher (the snapshot digest is checked while it is parsed: one pass, no copy).
struct HashRead<R> {
    inner: R,
    h: blake3::Hasher,
}

impl<R: io::Read> io::Read for HashRead<R> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(b)?;
        self.h.update(&b[..n]);
        Ok(n)
    }
}

enum SnapLoad {
    Loaded(Box<Snapshot>),
    /// The file is gone (pruned by a concurrent rotation, or deleted): replay the segments.
    Missing,
    /// Another snapshot version (e.g. v1 written by an older build): ignored, replay the segments.
    OtherVersion(String),
}

fn read_prefix(r: &mut impl io::Read, n: usize) -> io::Result<Vec<u8>> {
    let mut v = vec![0u8; n];
    let mut got = 0;
    while got < n {
        let k = r.read(&mut v[got..])?;
        if k == 0 {
            break;
        }
        got += k;
    }
    v.truncate(got);
    Ok(v)
}

/// The version word of a snapshot file prefix, if it is one (`rustred.campaign.snapshot.vN`).
fn snapshot_version(prefix: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(prefix).ok()?;
    let rest = s.strip_prefix("{\"schema\":\"")?;
    let word = &rest[..rest.find('"')?];
    word.starts_with(SNAPSHOT_SCHEMA_FAMILY)
        .then(|| word.to_string())
}

/// Streams the snapshot named by the manifest into a state, checking its BLAKE3 (manifest), schema and seq.
fn read_snapshot_checked(dir: &Path, s: &SnapshotRef) -> Result<SnapLoad, LedgerError> {
    let p = dir.join(&s.file);
    let f = match File::open(&p) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(SnapLoad::Missing),
        Err(e) => return Err(io_err(&p, "open snapshot")(e)),
    };
    let mut r = HashRead {
        inner: f,
        h: blake3::Hasher::new(),
    };
    let pre = read_prefix(&mut r, 64).map_err(io_err(&p, "read snapshot"))?;
    match snapshot_version(&pre) {
        Some(v) if v == SNAPSHOT_SCHEMA => {}
        Some(v) => return Ok(SnapLoad::OtherVersion(v)),
        None => return Err(corrupt(&s.file, 0, "not a ledger snapshot")),
    }
    let parsed: Result<Snapshot, serde_json::Error> = {
        let chained = io::Read::chain(&pre[..], &mut r);
        serde_json::from_reader(io::BufReader::with_capacity(1 << 20, chained))
    };
    io::copy(&mut r, &mut io::sink()).map_err(io_err(&p, "read snapshot"))?;
    if r.h.finalize().to_hex().as_str() != s.blake3 {
        return Err(corrupt(
            &s.file,
            0,
            "snapshot digest differs from the manifest",
        ));
    }
    let snap = parsed.map_err(|e| corrupt(&s.file, 0, e.to_string()))?;
    if snap.schema != SNAPSHOT_SCHEMA || snap.seq != s.seq {
        return Err(corrupt(&s.file, 0, "snapshot schema or seq mismatch"));
    }
    Ok(SnapLoad::Loaded(Box::new(snap)))
}

/// `verify`: does the manifest's snapshot hold exactly `st` (the state replayed to its seq, chain value `head`)?
/// Compared by digests while streaming the file (no second state in memory): the file's state bytes hash to the
/// manifest's `state_digest`, which equals the digest of `st`, and the header names the same chain value. A file
/// that differs from the manifest (BLAKE3, schema, seq) is an ERROR as in `open`; `None` when the file is gone
/// (pruned by a concurrent rotation) or of another snapshot version.
fn snapshot_matches_state(
    dir: &Path,
    sr: &SnapshotRef,
    head: &str,
    st: &CampaignState,
) -> Result<Option<bool>, LedgerError> {
    let p = dir.join(&sr.file);
    let mut f = match File::open(&p) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(io_err(&p, "open snapshot")(e)),
    };
    let len = f.metadata().map_err(io_err(&p, "stat snapshot"))?.len() as usize;
    let want = snapshot_prefix(sr.seq, head);
    let mut fh = blake3::Hasher::new();
    let mut sh = blake3::Hasher::new();
    let mut pre = Vec::with_capacity(want.len());
    let mut tail = Vec::new();
    let state_end = len.saturating_sub(2);
    let mut at = 0usize;
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let k = io::Read::read(&mut f, &mut buf).map_err(io_err(&p, "read snapshot"))?;
        if k == 0 {
            break;
        }
        let chunk = &buf[..k];
        fh.update(chunk);
        let p_end = want.len().saturating_sub(at).min(k);
        pre.extend_from_slice(&chunk[..p_end]);
        let s_end = state_end.saturating_sub(at).min(k).max(p_end);
        sh.update(&chunk[p_end..s_end]);
        tail.extend_from_slice(&chunk[s_end..]);
        at += k;
    }
    match snapshot_version(&pre) {
        Some(v) if v == SNAPSHOT_SCHEMA => {}
        Some(_) => return Ok(None),
        None => return Err(corrupt(&sr.file, 0, "not a ledger snapshot")),
    }
    if fh.finalize().to_hex().as_str() != sr.blake3 {
        return Err(corrupt(
            &sr.file,
            0,
            "snapshot digest differs from the manifest",
        ));
    }
    let head_at = want.len() - json_str(head).len() - "\"state\":".len() - 1;
    if pre.len() != want.len() || pre[..head_at] != want[..head_at] {
        return Err(corrupt(&sr.file, 0, "snapshot schema or seq mismatch"));
    }
    let region_ok = sh.finalize().to_hex().as_str() == sr.state_digest && tail == b"}\n";
    Ok(Some(
        pre == want && region_ok && sr.state_digest == st.digest(),
    ))
}

/// The state at the start of the active segment.
struct Base {
    state: CampaignState,
    seq: u64,
    head: String,
    from_snapshot: Option<u64>,
    replayed: u64,
    /// Why a snapshot named by the manifest was not used (missing / other version), if so.
    note: Option<String>,
}

/// State at the start of the active segment: the manifest's snapshot (it may end at ANY sealed segment boundary:
/// a background snapshot is referenced one rotation after its seal point) plus a replay of the sealed segments
/// after it, or a replay of all sealed segments. Every sealed segment is checked against the manifest (size and
/// BLAKE3; audit B exp2) before it is trusted. A missing snapshot file or one of another version means a full
/// replay (audit-ledger2 m2: a reader racing a rotation's prune never fails).
fn load_base(dir: &Path, m: &Manifest) -> Result<Base, LedgerError> {
    let mut note = None;
    let mut loaded: Option<(Box<Snapshot>, usize)> = None;
    if let Some(s) = &m.snapshot {
        let covered = m
            .sealed
            .iter()
            .position(|x| x.last_seq == s.seq)
            .map(|i| i + 1)
            .ok_or_else(|| {
                corrupt(
                    MANIFEST_FILE,
                    0,
                    format!("snapshot seq {} is not the end of a sealed segment", s.seq),
                )
            })?;
        let (mut cs, mut ch) = (0u64, GENESIS.to_string());
        for sealed in &m.sealed[..covered] {
            (cs, ch) = check_sealed(dir, sealed, cs, &ch)?;
        }
        match read_snapshot_checked(dir, s)? {
            SnapLoad::Loaded(snap) => {
                if snap.head != m.sealed[covered - 1].last_hash {
                    return Err(corrupt(
                        &s.file,
                        0,
                        "snapshot chain value differs from its sealed segment's last record",
                    ));
                }
                loaded = Some((snap, covered));
            }
            SnapLoad::Missing => {
                note = Some(format!("snapshot {} missing: full replay", s.file));
            }
            SnapLoad::OtherVersion(v) => {
                note = Some(format!(
                    "snapshot {} is {v}, this build reads {SNAPSHOT_SCHEMA}: full replay",
                    s.file
                ));
            }
        }
    }
    let (mut st, mut seq, mut head, from_snapshot, skip) = match loaded {
        Some((snap, covered)) => {
            let Snapshot {
                seq, head, state, ..
            } = *snap;
            (state, seq, head, Some(seq), covered)
        }
        None => (CampaignState::default(), 0, GENESIS.to_string(), None, 0),
    };
    let mut replayed = 0;
    for s in &m.sealed[skip..] {
        let (k, h) = replay_sealed(dir, s, seq, &head, &mut reducer(&mut st, None))?;
        replayed += k;
        seq = s.last_seq;
        head = h;
    }
    if seq + 1 != m.active.first_seq || head != m.active.prev {
        return Err(corrupt(
            MANIFEST_FILE,
            0,
            format!(
                "active segment starts at {} / {}, state ends at {seq} / {head}",
                m.active.first_seq, m.active.prev
            ),
        ));
    }
    Ok(Base {
        state: st,
        seq,
        head,
        from_snapshot,
        replayed,
        note,
    })
}

/// The commit mark restricted to the active segment, after checking it against the manifest.
fn mark_for_active<'a>(
    m: &Manifest,
    mark: Option<&'a CommitMark>,
) -> Result<Option<&'a CommitMark>, LedgerError> {
    let Some(mk) = mark else { return Ok(None) };
    if mk.segment > m.active.index {
        return Err(corrupt(
            COMMIT_FILE,
            0,
            format!(
                "commit mark names segment {} beyond the active segment {}",
                mk.segment, m.active.index
            ),
        ));
    }
    if mk.segment == m.active.index {
        return Ok(Some(mk));
    }
    if mk.seq >= m.active.first_seq {
        return Err(corrupt(
            COMMIT_FILE,
            0,
            format!(
                "commit mark of segment {} names seq {} beyond that segment",
                mk.segment, mk.seq
            ),
        ));
    }
    Ok(None)
}

/// A read-only view of a ledger (dashboard, website, verify): never repairs, truncates or writes.
#[derive(Debug)]
pub struct LedgerView {
    pub state: CampaignState,
    pub seq: u64,
    pub head: String,
    pub manifest: Manifest,
    pub mark: Option<CommitMark>,
    /// Bytes after the last complete valid record (a torn tail or a write in progress), with the reason.
    pub unverified_tail: Option<(u64, String)>,
}

/// Opens a ledger READ-ONLY (no lock, no repair). Order: commit mark, manifest, segments (so a concurrent writer
/// cannot make the mark look ahead of the data). A torn tail is reported, corruption is an error. Memory: one
/// reduced state (the snapshot is streamed).
pub fn open_read_only(cdir: &Path) -> Result<LedgerView, LedgerError> {
    let dir = ledger_dir(cdir);
    if !manifest_present(&dir)? {
        return Err(LedgerError::NotALedger { dir });
    }
    let mark = read_commit_mark(&dir)?;
    let manifest = read_manifest(&dir)?;
    let base = load_base(&dir, &manifest)?;
    let am = mark_for_active(&manifest, mark.as_ref())?;
    let bytes = read_segment_file(&dir, &manifest.active.file)?;
    let mut state = base.state;
    let scan = scan_active(
        &manifest.active.file,
        &bytes,
        base.seq,
        base.head,
        am,
        None,
        &mut reducer(&mut state, None),
    )?;
    if let Some((line, why)) = scan.corrupt {
        return Err(corrupt(&manifest.active.file, line, why));
    }
    Ok(LedgerView {
        state,
        seq: scan.seq,
        head: scan.head,
        unverified_tail: scan.torn.map(|why| ((bytes.len() - scan.pos) as u64, why)),
        manifest,
        mark,
    })
}

// ------------------------------------------------------------------------------------------------------------
// The ledger (write handle)
// ------------------------------------------------------------------------------------------------------------

pub struct Ledger {
    dir: PathBuf,
    cfg: LedgerConfig,
    clock: Arc<dyn Clock>,
    ops: Arc<dyn FileOps>,
    manifest: Manifest,
    file: Box<dyn SegmentFile>,
    mark_file: Box<dyn SegmentFile>,
    mark_counter: u64,
    active_len: u64,
    active_hasher: blake3::Hasher,
    seq: u64,
    head: String,
    state: CampaignState,
    unsynced: usize,
    last_sync: Duration,
    poisoned: Option<Poison>,
    warnings: Vec<String>,
    rotation_failed_at: Option<Duration>,
    /// The background snapshot of the last rotation (large states), and a finished one not yet in the manifest.
    snapshot_job: Option<SnapshotJob>,
    ready_snapshot: Option<SnapshotRef>,
    _dir_lock: File,
}

/// Why a handle is poisoned. `no_space`: the cause was ENOSPC / EDQUOT; `reported`: the causing error already
/// went to a caller (else the next call reports it with its type, then `Poisoned`; audit-ledger2 m1).
#[derive(Debug)]
struct Poison {
    why: String,
    no_space: Option<PathBuf>,
    reported: bool,
}

/// A background snapshot (state frozen at seq `seq`, the end of a sealed segment).
struct SnapshotJob {
    seq: u64,
    cancel: Arc<AtomicBool>,
    handle: std::thread::JoinHandle<Result<SnapshotRef, String>>,
}

impl std::fmt::Debug for Ledger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ledger")
            .field("dir", &self.dir)
            .field("seq", &self.seq)
            .field("head", &self.head)
            .finish()
    }
}

impl Drop for Ledger {
    /// ORDERLY CLOSE of a healthy handle: a running background snapshot is finished and named by the manifest
    /// (best effort: one atomic manifest install; the next `open` then replays little). A poisoned handle cancels
    /// it instead. Either way nothing writes into the directory after the handle is gone; an unreferenced snapshot
    /// file is removed by the next `open`.
    fn drop(&mut self) {
        let Some(job) = self.snapshot_job.take() else {
            return;
        };
        if self.poisoned.is_some() {
            job.cancel.store(true, Ordering::SeqCst);
            let _ = job.handle.join();
            return;
        }
        if self.collect_job(job).is_some()
            && let Some(r) = self.ready_snapshot.take()
            && self
                .manifest
                .snapshot
                .as_ref()
                .is_none_or(|cur| r.seq > cur.seq)
        {
            let mut m = self.manifest.clone();
            m.snapshot = Some(r);
            let _ = install(
                self.ops.as_ref(),
                &self.dir.join(MANIFEST_FILE),
                &manifest_bytes(&m),
            );
        }
    }
}

fn check_lock(cdir: &Path, lock: &CampaignLock) -> Result<(), LedgerError> {
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if canon(lock.dir()) != canon(cdir) || !lock.is_intact() {
        return Err(LedgerError::WrongLock {
            lock_dir: lock.dir().to_path_buf(),
            cdir: cdir.to_path_buf(),
        });
    }
    Ok(())
}

fn flock_dir(dir: &Path) -> Result<File, LedgerError> {
    let f = File::open(dir).map_err(io_err(dir, "open ledger dir"))?;
    // SAFETY: plain syscall on an owned, open descriptor.
    let r = unsafe { libc::flock(f.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if r != 0 {
        let e = io::Error::last_os_error();
        if e.raw_os_error() == Some(libc::EWOULDBLOCK) {
            return Err(LedgerError::Busy {
                dir: dir.to_path_buf(),
            });
        }
        return Err(io_err(dir, "flock ledger dir")(e));
    }
    Ok(f)
}

/// procfix7 round 5 (gate9 G9-B1): is `dir/MANIFEST.json` there? Ok(true): a regular file; Ok(false): PROVEN absent
/// (NotFound); Err: anything else (EIO, ENOMEM, EACCES, not a regular file): never "no ledger" (a stat error taken as
/// "no ledger" made a supervisor create a new ledger over a crashed campaign and delete its history).
fn manifest_present(dir: &Path) -> Result<bool, LedgerError> {
    let p = dir.join(MANIFEST_FILE);
    match crate::fsx::presence_lstat("ledger.manifest", &p) {
        crate::fsx::Presence::Present(m) if m.is_file() => Ok(true),
        crate::fsx::Presence::Present(_) => {
            Err(corrupt(MANIFEST_FILE, 0, "not a regular file".to_string()))
        }
        crate::fsx::Presence::Absent => Ok(false),
        crate::fsx::Presence::Unknown(e) => Err(io_err(&p, "stat manifest")(e)),
    }
}

/// [`manifest_present`] of a campaign directory's ledger (the supervisor's create-or-open decision).
pub fn manifest_state(cdir: &Path) -> Result<bool, LedgerError> {
    manifest_present(&ledger_dir(cdir))
}

/// procfix7 round 5: the proof a snapshot file may be removed (snapshots are derived data: the segments hold the
/// history), read back from the file just before: a regular file (lstat) in this ledger directory (held campaign lock
/// and directory flock) whose head is a snapshot (`{"schema":"rustred.campaign.snapshot.`...) of the seq its name
/// gives. Anything else (also an unreadable file) is kept.
fn snapshot_removable(path: &Path, seq: u64) -> bool {
    use std::io::Read as _;
    match crate::fsx::presence_lstat("ledger.snapshot", path) {
        crate::fsx::Presence::Present(m) if m.is_file() => {}
        _ => return false,
    }
    let mut head = vec![0u8; 512];
    let n = match File::open(path).and_then(|mut f| f.read(&mut head)) {
        Ok(n) => n,
        Err(_) => return false,
    };
    let head = String::from_utf8_lossy(&head[..n]);
    head.starts_with(&format!("{{\"schema\":\"{SNAPSHOT_SCHEMA_FAMILY}"))
        && head.contains(&format!("\"seq\":{seq},"))
}

#[derive(Serialize, Deserialize)]
struct PendingRecovery {
    info: RecoveryInfo,
    truncate_to: u64,
}

fn segment_index(name: &str) -> Option<u32> {
    name.strip_prefix("segment-")?
        .strip_suffix(".jsonl")?
        .parse()
        .ok()
}

/// What [`rebuild_manifest`] did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RebuildReport {
    pub sealed: u32,
    pub records: u64,
    pub active: String,
}

/// RECOVERY VERB for a damaged ledger without MANIFEST.json (M1): rebuilds the manifest from the segments found,
/// after verifying the WHOLE chain from record 1 (framing, digests, canonical form, dense seq, `prev` chain, the
/// reducer accepts every record, the commit mark lies within the data and names a record boundary) and the
/// campaign id. Segments must be numbered 1..=N without a gap; the last one becomes the active segment, the
/// others sealed (size, BLAKE3, last hash recomputed); no snapshot (the next rotation writes one); nothing is
/// deleted or rewritten except the new manifest. Needs the held campaign lock. Refuses (typed `Corrupt`) on any
/// doubt; refuses `AlreadyExists` when a manifest is present.
pub fn rebuild_manifest(cdir: &Path, lock: &CampaignLock) -> Result<RebuildReport, LedgerError> {
    check_lock(cdir, lock)?;
    let dir = ledger_dir(cdir);
    if manifest_present(&dir)? {
        return Err(LedgerError::AlreadyExists { dir });
    }
    let _dir_lock = flock_dir(&dir)?;
    // procfix7 round 5: a COMPLETE listing (an entry error fails the rebuild, never a shorter chain)
    let mut idx: Vec<u32> = crate::fsx::list_required("ledger.listing", &dir)
        .map_err(io_err(&dir, "list"))?
        .iter()
        .filter_map(|e| segment_index(&e.name_lossy()))
        .collect();
    idx.sort_unstable();
    if idx.is_empty() || idx.iter().enumerate().any(|(i, &x)| x as usize != i + 1) {
        return Err(corrupt(
            "segment-*",
            0,
            format!("segments must be numbered 1..=N without a gap, found {idx:?}"),
        ));
    }
    let mark = read_commit_mark(&dir)?;
    let mut st = CampaignState::default();
    let (mut seq, mut head) = (0u64, GENESIS.to_string());
    let mut sealed = Vec::new();
    let last = *idx.last().unwrap();
    let mut active: Option<ActiveSegment> = None;
    for &i in &idx {
        let file = segment_name(i);
        let bytes = read_segment_file(&dir, &file)?;
        let (first_seq, prev) = (seq + 1, head.clone());
        if i < last {
            let mut pos = 0usize;
            let mut n = 0u64;
            while pos < bytes.len() {
                let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
                    return Err(corrupt(&file, n + 1, "sealed segment ends inside a line"));
                };
                let (rec, h) =
                    decode_record(&bytes[pos..pos + nl]).map_err(|w| corrupt(&file, n + 1, w))?;
                if rec.seq != seq + 1 || rec.prev != head {
                    return Err(corrupt(&file, n + 1, "sequence or hash chain broken"));
                }
                st.apply_record(&rec).map_err(LedgerError::Replay)?;
                seq = rec.seq;
                head = h;
                n += 1;
                pos += nl + 1;
            }
            if n == 0 {
                return Err(corrupt(&file, 0, "empty segment before the last one"));
            }
            sealed.push(SealedSegment {
                index: i,
                file,
                first_seq,
                last_seq: seq,
                bytes: bytes.len() as u64,
                blake3: blake3::hash(&bytes).to_hex().to_string(),
                last_hash: head.clone(),
            });
        } else {
            let am = mark.as_ref().filter(|m| m.segment == i);
            if mark
                .as_ref()
                .is_some_and(|m| m.segment > i || (m.segment < i && m.seq >= first_seq))
            {
                return Err(corrupt(
                    COMMIT_FILE,
                    0,
                    "commit mark names records beyond the segments found",
                ));
            }
            let scan = scan_active(
                &file,
                &bytes,
                seq,
                head.clone(),
                am,
                None,
                &mut reducer(&mut st, None),
            )?;
            if let Some((line, why)) = scan.corrupt {
                return Err(corrupt(&file, line, why));
            }
            seq = scan.seq;
            active = Some(ActiveSegment {
                index: i,
                file,
                first_seq,
                prev,
            });
        }
    }
    if st.campaign_id.is_empty() {
        return Err(corrupt(
            "segment-000001.jsonl",
            1,
            "no campaign_created record",
        ));
    }
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA.into(),
        campaign_id: st.campaign_id.clone(),
        sealed,
        active: active.expect("last segment"),
        snapshot: None,
    };
    let ops = RealOps;
    install(&ops, &dir.join(MANIFEST_FILE), &manifest_bytes(&manifest))
        .map_err(|f| install_err(&dir.join(MANIFEST_FILE), f))?;
    Ok(RebuildReport {
        sealed: manifest.sealed.len() as u32,
        records: seq,
        active: manifest.active.file,
    })
}

/// Background snapshot writer: streams the frozen state into `snapshot-<seq>.json` (temp + fsync + rename + dir
/// fsync); cancellable (the handle is being dropped).
fn background_snapshot(
    ops: Arc<dyn FileOps>,
    dir: PathBuf,
    seq: u64,
    head: String,
    state: CampaignState,
    cancel: Arc<AtomicBool>,
) -> Result<SnapshotRef, String> {
    struct CancelWrite<'a> {
        w: &'a mut dyn Write,
        cancel: &'a AtomicBool,
    }
    impl Write for CancelWrite<'_> {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> {
            if self.cancel.load(Ordering::Relaxed) {
                return Err(io::Error::other("background snapshot cancelled"));
            }
            self.w.write(b)
        }
        fn flush(&mut self) -> io::Result<()> {
            self.w.flush()
        }
    }
    let name = snapshot_name(seq);
    let path = dir.join(&name);
    let n = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = dir.join(format!(".{name}.rr-tmp-{}-{n}", std::process::id()));
    let mut digests = None;
    let r = ops.write_new_synced_with(&temp, &mut |w| {
        let mut cw = CancelWrite { w, cancel: &cancel };
        digests = Some(write_snapshot_to(&mut cw, seq, &head, &state)?);
        Ok(())
    });
    drop(state);
    let fail = |what: &str, e: io::Error| {
        let _ = fs::remove_file(&temp);
        format!("{what} {name}: {e}")
    };
    r.map_err(|e| fail("write", e))?;
    if cancel.load(Ordering::SeqCst) {
        let _ = fs::remove_file(&temp);
        return Err("cancelled".into());
    }
    ops.rename(&temp, &path).map_err(|e| fail("rename", e))?;
    ops.sync_dir(&dir)
        .map_err(|e| format!("sync dir after {name}: {e}"))?;
    let (blake3, state_digest) = digests.expect("written");
    Ok(SnapshotRef {
        file: name,
        seq,
        blake3,
        state_digest,
    })
}

impl Ledger {
    /// Creates a new ledger in `cdir/ledger` and writes record 1 (`campaign_created`). Record 1 is written and
    /// fsynced BEFORE the manifest is installed (the commit point). procfix7 round 5 (gate9 G9-B1): a new ledger is
    /// created ONLY into a ledger directory PROVEN absent or PROVEN empty (a complete listing); a directory that holds
    /// anything without a manifest (also the residue of a crashed create) is refused with `ManifestMissing` and NOTHING
    /// is deleted (never a repair by deletion; recovery: `rebuild_manifest`, or removal by a person); a manifest whose
    /// presence cannot be read is an error, never "no ledger".
    pub fn create(
        cdir: &Path,
        lock: &CampaignLock,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
        genesis: Genesis,
    ) -> Result<Ledger, LedgerError> {
        Self::create_with(cdir, lock, clock, cfg, Arc::new(RealOps), genesis)
    }

    pub fn create_with(
        cdir: &Path,
        lock: &CampaignLock,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
        ops: Arc<dyn FileOps>,
        genesis: Genesis,
    ) -> Result<Ledger, LedgerError> {
        check_lock(cdir, lock)?;
        let dir = ledger_dir(cdir);
        if manifest_present(&dir)? {
            return Err(LedgerError::AlreadyExists { dir });
        }
        let fresh = match crate::fsx::presence_lstat("ledger.dir", &dir) {
            crate::fsx::Presence::Absent => true,
            crate::fsx::Presence::Present(m) if m.is_dir() => false,
            crate::fsx::Presence::Present(_) => {
                return Err(corrupt("ledger", 0, "not a directory".to_string()));
            }
            crate::fsx::Presence::Unknown(e) => return Err(io_err(&dir, "stat ledger dir")(e)),
        };
        if fresh {
            fs::create_dir(&dir).map_err(io_err(&dir, "create dir"))?;
            ops.sync_dir(cdir).map_err(io_err(cdir, "sync dir"))?;
        }
        let dir_lock = flock_dir(&dir)?;
        // PROVEN empty (a complete listing, under the held campaign lock and the directory flock), else refused
        match crate::fsx::list("ledger.listing", &dir) {
            crate::fsx::Listing::Complete(v) if v.is_empty() => {}
            crate::fsx::Listing::Complete(v) => {
                let mut found: Vec<String> = v.iter().map(|e| e.name_lossy()).collect();
                found.sort();
                return Err(LedgerError::ManifestMissing {
                    dir,
                    found: found.join(", "),
                });
            }
            crate::fsx::Listing::Absent => {
                return Err(io_err(&dir, "list")(io::Error::from_raw_os_error(
                    libc::ENOENT,
                )));
            }
            crate::fsx::Listing::Failed(e) => return Err(io_err(&dir, "list")(e)),
        }
        let manifest = Manifest {
            schema: MANIFEST_SCHEMA.into(),
            campaign_id: genesis.campaign_id.clone(),
            sealed: Vec::new(),
            active: ActiveSegment {
                index: 1,
                file: segment_name(1),
                first_seq: 1,
                prev: GENESIS.into(),
            },
            snapshot: None,
        };
        let mark_path = dir.join(COMMIT_FILE);
        let mut mark_file = ops
            .open_mark(&mark_path)
            .map_err(io_err(&mark_path, "create commit mark"))?;
        mark_file
            .write_at(0, &[b' '; 2 * MARK_SLOT_BYTES])
            .map_err(io_err(&mark_path, "init commit mark"))?;
        let seg = dir.join(&manifest.active.file);
        let file = ops
            .open_segment(&seg)
            .map_err(io_err(&seg, "create segment"))?;
        let now = clock.monotonic();
        let mut l = Ledger {
            dir: dir.clone(),
            cfg,
            clock,
            ops,
            manifest,
            file,
            mark_file,
            mark_counter: 0,
            active_len: 0,
            active_hasher: blake3::Hasher::new(),
            seq: 0,
            head: GENESIS.into(),
            state: CampaignState::default(),
            unsynced: 0,
            last_sync: now,
            poisoned: None,
            warnings: Vec::new(),
            rotation_failed_at: None,
            snapshot_job: None,
            ready_snapshot: None,
            _dir_lock: dir_lock,
        };
        l.append(Event::CampaignCreated {
            campaign_id: genesis.campaign_id,
            spec_blake3: genesis.spec_blake3,
            terminal_bound: genesis.terminal_bound,
            mode: genesis.mode,
        })?;
        l.ops.sync_dir(&dir).map_err(io_err(&dir, "sync dir"))?;
        let mbytes = manifest_bytes(&l.manifest);
        install(l.ops.as_ref(), &dir.join(MANIFEST_FILE), &mbytes)
            .map_err(|f| install_err(&dir.join(MANIFEST_FILE), f))?;
        Ok(l)
    }

    /// Opens an existing ledger for writing (see the module doc: recovery rules, fsync gate).
    pub fn open(
        cdir: &Path,
        lock: &CampaignLock,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
    ) -> Result<(Ledger, OpenReport), LedgerError> {
        Self::open_with(
            cdir,
            lock,
            clock,
            cfg,
            Arc::new(RealOps),
            OpenOptions::default(),
        )
    }

    pub fn open_with(
        cdir: &Path,
        lock: &CampaignLock,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
        ops: Arc<dyn FileOps>,
        opts: OpenOptions,
    ) -> Result<(Ledger, OpenReport), LedgerError> {
        check_lock(cdir, lock)?;
        let dir = ledger_dir(cdir);
        if !manifest_present(&dir)? {
            return Err(LedgerError::NotALedger { dir });
        }
        let dir_lock = flock_dir(&dir)?;
        let manifest = read_manifest(&dir)?;
        let mut report = OpenReport {
            removed_unreferenced: remove_unreferenced(&dir, &manifest)?,
            ..OpenReport::default()
        };
        let mark = read_commit_mark(&dir)?;
        // procfix7 round 5: an unreadable pending-recovery file is an error, never "no recovery pending"
        let pending: Option<PendingRecovery> =
            match crate::fsx::read("ledger.segment", &dir.join(RECOVERY_PENDING_FILE)) {
                crate::fsx::Read::Content(b) => Some(
                    serde_json::from_slice(&b)
                        .map_err(|e| corrupt(RECOVERY_PENDING_FILE, 0, e.to_string()))?,
                ),
                crate::fsx::Read::Absent => None,
                crate::fsx::Read::Failed(e) => {
                    return Err(io_err(&dir.join(RECOVERY_PENDING_FILE), "read")(e));
                }
            };
        let base = load_base(&dir, &manifest)?;
        report.from_snapshot = base.from_snapshot;
        report.replayed = base.replayed;
        report.snapshot_note = base.note.clone();
        let (base_seq, base_head) = (base.seq, base.head.clone());
        let am = mark_for_active(&manifest, mark.as_ref())?.cloned();
        let seg_path = dir.join(&manifest.active.file);
        let bytes = read_segment_file(&dir, &manifest.active.file)?;
        let mut state = base.state;
        let mut scan = scan_active(
            &manifest.active.file,
            &bytes,
            base_seq,
            base_head.clone(),
            am.as_ref(),
            None,
            &mut reducer(&mut state, None),
        )?;
        let mut recovery: Option<(RecoveryInfo, u64)> = None;
        if let Some((line, why)) = scan.corrupt.take() {
            let Some(to) = opts.accept_truncation_to else {
                return Err(corrupt(&manifest.active.file, line, why));
            };
            if to < base_seq || to > scan.seq {
                return Err(corrupt(
                    &manifest.active.file,
                    line,
                    format!(
                        "{why}; override --accept-truncation-to {to} is outside the active segment's good records {base_seq}..={}",
                        scan.seq
                    ),
                ));
            }
            // the override replays the active segment again from the base state (rare: reload it, no copy kept)
            state = load_base(&dir, &manifest)?.state;
            scan = scan_active(
                &manifest.active.file,
                &bytes,
                base_seq,
                base_head,
                None,
                Some(to),
                &mut reducer(&mut state, None),
            )?;
            recovery = Some((
                RecoveryInfo {
                    segment: manifest.active.index,
                    last_good_seq: scan.seq,
                    truncated_bytes: (bytes.len() - scan.pos) as u64,
                    saved_as: None,
                    detail: format!("operator override --accept-truncation-to {to}: {why}"),
                    forced: true,
                },
                scan.pos as u64,
            ));
        } else if let Some(why) = scan.torn.take() {
            recovery = Some((
                RecoveryInfo {
                    segment: manifest.active.index,
                    last_good_seq: scan.seq,
                    truncated_bytes: (bytes.len() - scan.pos) as u64,
                    saved_as: None,
                    detail: why,
                    forced: false,
                },
                scan.pos as u64,
            ));
        }
        let mark_path = dir.join(COMMIT_FILE);
        let mark_file = ops
            .open_mark(&mark_path)
            .map_err(io_err(&mark_path, "open commit mark"))?;
        let file = ops
            .open_segment(&seg_path)
            .map_err(io_err(&seg_path, "open segment"))?;
        let now_mono = clock.monotonic();
        let mut l = Ledger {
            dir: dir.clone(),
            cfg,
            clock,
            ops,
            manifest,
            file,
            mark_file,
            mark_counter: mark.as_ref().map(|m| m.counter).unwrap_or(0),
            active_len: scan.pos as u64,
            active_hasher: scan.hasher,
            seq: scan.seq,
            head: scan.head,
            state,
            unsynced: 0,
            last_sync: now_mono,
            poisoned: None,
            warnings: Vec::new(),
            rotation_failed_at: None,
            snapshot_job: None,
            ready_snapshot: None,
            _dir_lock: dir_lock,
        };
        report.replayed += scan.replayed;
        let mark_off = am.as_ref().map(|m| m.offset).unwrap_or(0);
        // Truncation (torn tail or override): marker first (crash window, audit A m5), dropped bytes saved.
        let mut finish: Option<RecoveryInfo> = None;
        if let Some((mut info, truncate_to)) = recovery {
            let dropped = &bytes[truncate_to as usize..];
            let saved = match &pending {
                Some(p) if p.truncate_to == truncate_to => p.info.saved_as.clone(),
                _ => {
                    let name = format!(
                        "torn-{:012}-{}.bin",
                        info.last_good_seq,
                        l.clock.now_utc().0
                    );
                    let p = dir.join(&name);
                    l.ops
                        .write_new_synced(&p, dropped)
                        .map_err(io_err(&p, "save torn bytes"))?;
                    Some(name)
                }
            };
            info.saved_as = saved;
            let marker = PendingRecovery {
                info: info.clone(),
                truncate_to,
            };
            let mb = serde_json::to_vec(&marker).expect("marker serialises");
            install(l.ops.as_ref(), &dir.join(RECOVERY_PENDING_FILE), &mb)
                .map_err(|f| install_err(&dir.join(RECOVERY_PENDING_FILE), f))?;
            l.file
                .truncate(truncate_to)
                .map_err(io_err(&seg_path, "truncate tail"))?;
            report.recovery = Some(info.clone());
            finish = Some(info);
        } else if let Some(p) = pending {
            // The truncation happened before a crash; its record may be missing.
            if !l
                .state
                .recoveries
                .contains(&p.info.saved_as.clone().unwrap_or_default())
            {
                finish = Some(p.info.clone());
            } else {
                let _ = l.ops.remove(&dir.join(RECOVERY_PENDING_FILE));
            }
            report.recovery = Some(p.info);
        }
        // Fsync gate BEFORE any new record (audit-ledger2 m11): rewrite and fsync every byte beyond the commit
        // mark (pages a failed fsync left "clean but not on disk" become dirty again), then advance the mark; only
        // then may a recovery record's mark cover them.
        if l.active_len > mark_off {
            let end = (l.active_len as usize).min(bytes.len());
            let tail = &bytes[mark_off as usize..end];
            l.file
                .write_at(mark_off, tail)
                .map_err(io_err(&seg_path, "fsync gate rewrite"))?;
            l.file.sync().map_err(io_err(&seg_path, "fsync gate"))?;
            report.regated_bytes = tail.len() as u64;
            l.write_mark()
                .map_err(io_err(&dir.join(COMMIT_FILE), "commit mark"))?;
        }
        if let Some(info) = finish {
            l.finish_recovery(&info)?;
        }
        Ok((l, report))
    }

    /// Appends the recovery record of a truncation and removes the pending marker.
    fn finish_recovery(&mut self, info: &RecoveryInfo) -> Result<(), LedgerError> {
        self.append(Event::Recovery {
            segment: info.segment,
            last_good_seq: info.last_good_seq,
            truncated_bytes: info.truncated_bytes,
            saved_as: info.saved_as.clone(),
            detail: info.detail.clone(),
            forced: info.forced,
        })?;
        let p = self.dir.join(RECOVERY_PENDING_FILE);
        self.ops
            .remove(&p)
            .map_err(io_err(&p, "remove recovery marker"))?;
        self.ops
            .sync_dir(&self.dir)
            .map_err(io_err(&self.dir, "sync dir"))?;
        Ok(())
    }

    /// The reduced state of every ACKNOWLEDGED record (after a write error it is rebuilt from disk without the
    /// failed batch; audit A m3).
    pub fn state(&self) -> &CampaignState {
        &self.state
    }
    pub fn seq(&self) -> u64 {
        self.seq
    }
    pub fn head(&self) -> &str {
        &self.head
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }
    pub fn is_poisoned(&self) -> bool {
        self.poisoned.is_some()
    }
    /// Non-fatal conditions after acknowledged appends (failed rotation, failed background snapshot).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }
    /// True while a background snapshot is being written.
    pub fn snapshot_in_progress(&self) -> bool {
        self.snapshot_job
            .as_ref()
            .is_some_and(|j| !j.handle.is_finished())
    }

    /// Waits for the background snapshot of the last rotation (if any) and keeps its reference for the next
    /// manifest (tests, orderly shutdown; appends never need this). Returns its seq when one finished.
    pub fn wait_snapshot(&mut self) -> Option<u64> {
        let job = self.snapshot_job.take()?;
        self.collect_job(job)
    }

    fn collect_job(&mut self, job: SnapshotJob) -> Option<u64> {
        match job.handle.join() {
            Ok(Ok(r)) => {
                let seq = r.seq;
                self.ready_snapshot = Some(r);
                Some(seq)
            }
            Ok(Err(e)) => {
                self.warnings.push(format!(
                    "background snapshot at seq {} failed: {e}",
                    job.seq
                ));
                None
            }
            Err(_) => {
                self.warnings
                    .push(format!("background snapshot at seq {} panicked", job.seq));
                None
            }
        }
    }

    fn poison(&mut self, why: String, cause: Option<&LedgerError>, reported: bool) {
        let no_space = match cause {
            Some(LedgerError::NoSpace { path }) => Some(path.clone()),
            _ => None,
        };
        self.poisoned = Some(Poison {
            why,
            no_space,
            reported,
        });
    }

    /// The error a poisoned handle returns: the causing ENOSPC once with its type if no caller saw it yet (a
    /// failure inside a rotation is only a warning of the append that triggered it), else `Poisoned`.
    fn poisoned_err(&mut self) -> Option<LedgerError> {
        let p = self.poisoned.as_mut()?;
        if !p.reported {
            p.reported = true;
            if let Some(path) = &p.no_space {
                return Some(LedgerError::NoSpace { path: path.clone() });
            }
        }
        Some(LedgerError::Poisoned { why: p.why.clone() })
    }

    /// Appends one record (validated by the reducer first: an illegal record is never written). Returns its seq.
    pub fn append(&mut self, event: Event) -> Result<u64, LedgerError> {
        let s = self.append_all(vec![event])?;
        Ok(s[0])
    }

    /// Appends several records with ONE fsync at the end if any of them is `Sync` (group commit). All or nothing
    /// against the reducer. `Ok` = every record is written (and durable if any is `Sync`, the commit mark naming
    /// it); a later rotation failure is a warning, never an error of this call (audit A M2).
    pub fn append_all(&mut self, events: Vec<Event>) -> Result<Vec<u64>, LedgerError> {
        if let Some(e) = self.poisoned_err() {
            return Err(e);
        }
        let start_len = self.active_len;
        let mut seqs = Vec::with_capacity(events.len());
        let mut need_sync = false;
        let mut buf = Vec::new();
        // No clone of the state per batch (O(state) per group commit at scale): a record refused after earlier
        // records of the same batch were applied rebuilds the state from disk (the batch is not written yet).
        let restore = |l: &mut Ledger, applied: usize| {
            if applied > 0 {
                match l.rebuild_state(l.active_len) {
                    Ok(st) => l.state = st,
                    Err(e) => l.poison(
                        format!("batch refused and state rebuild failed: {e}"),
                        None,
                        false,
                    ),
                }
            }
        };
        let (mut seq, mut head) = (self.seq, self.head.clone());
        for ev in events {
            need_sync |= ev.durability() == Durability::Sync;
            let rec = Record {
                schema: LEDGER_SCHEMA.into(),
                seq: seq + 1,
                utc: self.clock.now_utc().to_iso(),
                prev: head.clone(),
                event: ev,
            };
            let (line, h) = encode_record(&rec);
            if line.len() > MAX_RECORD_BYTES {
                restore(self, seqs.len());
                return Err(LedgerError::TooLarge { bytes: line.len() });
            }
            if let Err(e) = self.state.apply_record(&rec) {
                restore(self, seqs.len());
                return Err(LedgerError::Refused(e));
            }
            seq = rec.seq;
            head = h;
            buf.extend_from_slice(&line);
            buf.push(b'\n');
            seqs.push(seq);
        }
        let path = self.dir.join(&self.manifest.active.file);
        if let Err(e) = self.file.append(&buf) {
            let err = io_err(&path, "append")(e);
            self.fail_write(start_len, &err);
            return Err(err);
        }
        let n = seqs.len();
        let now = self.clock.monotonic();
        let due = self.unsynced + n >= self.cfg.lazy_max_records
            || now.saturating_sub(self.last_sync) >= Duration::from_millis(self.cfg.lazy_max_ms);
        if (need_sync || due)
            && let Err(e) = self.file.sync()
        {
            let err = io_err(&path, "fsync")(e);
            self.fail_write(start_len, &err);
            return Err(err);
        }
        self.active_len += buf.len() as u64;
        self.active_hasher.update(&buf);
        self.seq = seq;
        self.head = head;
        if need_sync || due {
            self.unsynced = 0;
            self.last_sync = now;
            // audit-ledger2 m7: a failed mark write poisons and the batch is NOT acknowledged, so no acknowledged
            // record ever lies beyond the on-disk mark (open may then cut any torn-shaped damage beyond it). The
            // batch itself is durable (fsynced) and stays: a partly written slot may still be a valid mark naming it,
            // so it is neither truncated nor removed from the state (after reopen the ledger is the truth).
            if let Err(e) = self.write_mark() {
                let err = io_err(&self.dir.join(COMMIT_FILE), "commit mark")(e);
                self.poison(
                    format!("{err}; batch durable, not acknowledged"),
                    Some(&err),
                    true,
                );
                return Err(err);
            }
        } else {
            self.unsynced += n;
        }
        if self.active_len >= self.cfg.rotate_bytes {
            let retry_ok = self.rotation_failed_at.is_none_or(|t| {
                now.saturating_sub(t) >= Duration::from_millis(self.cfg.rotation_retry_ms)
            });
            let was_poisoned = self.poisoned.is_some();
            if retry_ok && let Err(e) = self.rotate() {
                self.rotation_failed_at = Some(now);
                self.warnings.push(format!(
                    "rotation failed after record {seq} (retried later): {e}"
                ));
                // the caller of THIS append sees Ok: the poisoning cause is reported by the next call
                if !was_poisoned && let Some(p) = self.poisoned.as_mut() {
                    p.reported = false;
                }
            }
        }
        Ok(seqs)
    }

    /// A failed write / fsync: truncate the batch away (best effort), poison, rebuild the state from disk.
    fn fail_write(&mut self, start_len: u64, err: &LedgerError) {
        let truncated = self.file.truncate(start_len).is_ok();
        let why = format!(
            "{err}; batch {}",
            if truncated {
                "truncated away"
            } else {
                "may remain on disk"
            }
        );
        self.poison(why, Some(err), true);
        match self.rebuild_state(start_len) {
            Ok(st) => self.state = st,
            Err(e) => {
                let p = self.poisoned.as_mut().expect("just poisoned");
                p.why = format!("{}; state rebuild failed too: {e}", p.why);
            }
        }
    }

    /// The state of the records in the active segment's first `len` bytes (from disk).
    fn rebuild_state(&self, len: u64) -> Result<CampaignState, LedgerError> {
        let base = load_base(&self.dir, &self.manifest)?;
        let bytes = read_segment_file(&self.dir, &self.manifest.active.file)?;
        let cut = (len as usize).min(bytes.len());
        let mut st = base.state;
        scan_active(
            &self.manifest.active.file,
            &bytes[..cut],
            base.seq,
            base.head,
            None,
            None,
            &mut reducer(&mut st, None),
        )?;
        Ok(st)
    }

    /// Writes and fsyncs the commit mark at the current end (after a successful segment fsync).
    fn write_mark(&mut self) -> io::Result<()> {
        self.mark_counter += 1;
        let m = CommitMark {
            counter: self.mark_counter,
            segment: self.manifest.active.index,
            seq: self.seq,
            offset: self.active_len,
            head: self.head.clone(),
        };
        let slot = (self.mark_counter % 2) * MARK_SLOT_BYTES as u64;
        self.mark_file
            .write_at(slot, &m.encode())
            .and_then(|_| self.mark_file.sync())
    }

    /// fsync of the active segment (makes lazy records durable), then the commit mark. A failure poisons.
    pub fn sync(&mut self) -> Result<(), LedgerError> {
        if let Some(e) = self.poisoned_err() {
            return Err(e);
        }
        let path = self.dir.join(&self.manifest.active.file);
        if let Err(e) = self.file.sync() {
            let err = io_err(&path, "fsync")(e);
            self.poison(format!("{err}"), Some(&err), true);
            return Err(err);
        }
        self.unsynced = 0;
        self.last_sync = self.clock.monotonic();
        if let Err(e) = self.write_mark() {
            let err = io_err(&self.dir.join(COMMIT_FILE), "commit mark")(e);
            self.poison(format!("{err}"), Some(&err), true);
            return Err(err);
        }
        Ok(())
    }

    /// Seals the active segment, makes a snapshot, starts a new segment. Steps (the manifest rename is the commit
    /// point; a crash before it leaves unreferenced files that `open` removes): (1) fsync active + mark; (2) the
    /// snapshot: INLINE for a small state (file written now, named by this manifest), else the BACKGROUND snapshot
    /// started at the previous rotation is named if it has finished (never waited for) and a new one is started
    /// after the manifest rename from a frozen copy of the state (copy-on-write: milliseconds); (3) new empty
    /// segment + dir fsync; (4) manifest (atomic); (5) old segment chmod 0444; (6) `segment_opened` record in the
    /// new segment; (7) prune snapshots. An error before the manifest rename leaves the ledger as it was (the
    /// caller may retry); an error after it poisons the handle.
    pub fn rotate(&mut self) -> Result<(), LedgerError> {
        if let Some(e) = self.poisoned_err() {
            return Err(e);
        }
        if self.active_len == 0 {
            return Ok(());
        }
        self.sync()?;
        let old = self.manifest.active.clone();
        let seg_blake3 = self.active_hasher.clone().finalize().to_hex().to_string();
        let background = self.state.items.len() + self.state.jobs.len()
            >= self.cfg.background_snapshot_min_entries;
        let mut inline_snapshot = None;
        if background {
            if let Some(job) = self.snapshot_job.take() {
                let snap_seq = self.manifest.snapshot.as_ref().map_or(0, |s| s.seq);
                let lag = 1 + self
                    .manifest
                    .sealed
                    .iter()
                    .filter(|x| x.last_seq > snap_seq)
                    .count();
                if job.handle.is_finished() || lag > self.cfg.snapshot_max_lag_segments {
                    self.collect_job(job); // waits only when snapshots fall that far behind
                } else {
                    self.snapshot_job = Some(job); // still writing: keep it, start none now
                }
            }
        } else {
            let mut snap_bytes = Vec::new();
            let (blake3, state_digest) =
                write_snapshot_to(&mut snap_bytes, self.seq, &self.head, &self.state)
                    .expect("in-memory snapshot");
            let snap_name = snapshot_name(self.seq);
            let snap_path = self.dir.join(&snap_name);
            install(self.ops.as_ref(), &snap_path, &snap_bytes)
                .map_err(|f| install_err(&snap_path, f))?;
            inline_snapshot = Some(SnapshotRef {
                file: snap_name,
                seq: self.seq,
                blake3,
                state_digest,
            });
        }
        let new_index = old.index + 1;
        let new_name = segment_name(new_index);
        let new_path = self.dir.join(&new_name);
        let new_file = self
            .ops
            .open_segment(&new_path)
            .map_err(io_err(&new_path, "create segment"))?;
        self.ops
            .sync_dir(&self.dir)
            .map_err(io_err(&self.dir, "sync dir"))?;
        let mut m = self.manifest.clone();
        m.sealed.push(SealedSegment {
            index: old.index,
            file: old.file.clone(),
            first_seq: old.first_seq,
            last_seq: self.seq,
            bytes: self.active_len,
            blake3: seg_blake3.clone(),
            last_hash: self.head.clone(),
        });
        m.active = ActiveSegment {
            index: new_index,
            file: new_name,
            first_seq: self.seq + 1,
            prev: self.head.clone(),
        };
        let named = inline_snapshot.or_else(|| self.ready_snapshot.clone());
        if let Some(r) = &named
            && m.snapshot.as_ref().is_none_or(|cur| r.seq > cur.seq)
        {
            m.snapshot = Some(r.clone());
        }
        let mpath = self.dir.join(MANIFEST_FILE);
        match install(self.ops.as_ref(), &mpath, &manifest_bytes(&m)) {
            Ok(()) => {}
            Err(InstallFail::BeforeRename(e)) => return Err(io_err(&mpath, "install manifest")(e)),
            Err(InstallFail::AfterRename(e)) => {
                let err = io_err(&mpath, "sync dir after manifest rename")(e);
                self.poison(
                    format!("{err}: the on-disk manifest may name the new segment"),
                    Some(&err),
                    true,
                );
                return Err(err);
            }
        }
        self.manifest = m;
        self.ready_snapshot = None;
        let _ = fs::set_permissions(self.dir.join(&old.file), fs::Permissions::from_mode(0o444));
        self.file = new_file;
        self.active_len = 0;
        self.active_hasher = blake3::Hasher::new();
        let snapshot_seq = self.seq;
        if background && self.snapshot_job.is_none() {
            // freeze (copy-on-write maps: keys + one reference count per item / job) and write off the append path
            let frozen = self.state.clone();
            let (ops, dir, head) = (self.ops.clone(), self.dir.clone(), self.head.clone());
            let cancel = Arc::new(AtomicBool::new(false));
            let c2 = cancel.clone();
            match std::thread::Builder::new()
                .name("rr-ledger-snapshot".into())
                .spawn(move || background_snapshot(ops, dir, snapshot_seq, head, frozen, c2))
            {
                Ok(handle) => {
                    self.snapshot_job = Some(SnapshotJob {
                        seq: snapshot_seq,
                        cancel,
                        handle,
                    })
                }
                Err(e) => self
                    .warnings
                    .push(format!("background snapshot not started: {e}")),
            }
        }
        self.append(Event::SegmentOpened {
            index: new_index,
            previous_blake3: seg_blake3,
            snapshot_seq,
        })?;
        prune_snapshots(&self.dir, &self.manifest, self.cfg.keep_old_snapshots);
        Ok(())
    }
}

fn manifest_bytes(m: &Manifest) -> Vec<u8> {
    let mut b = serde_json::to_vec(m).expect("manifest serialises");
    b.push(b'\n');
    b
}

pub fn read_manifest(dir: &Path) -> Result<Manifest, LedgerError> {
    let p = dir.join(MANIFEST_FILE);
    if !manifest_present(dir)? {
        return Err(LedgerError::NotALedger {
            dir: dir.to_path_buf(),
        });
    }
    let bytes = atomic::read_capped(&p, 64 << 20)?;
    let m: Manifest =
        serde_json::from_slice(&bytes).map_err(|e| corrupt(MANIFEST_FILE, 0, e.to_string()))?;
    if m.schema != MANIFEST_SCHEMA {
        return Err(corrupt(MANIFEST_FILE, 0, format!("schema {:?}", m.schema)));
    }
    Ok(m)
}

/// Removes files a crash during rotation left behind: EMPTY segment files with an index above the active one,
/// and snapshot files not in the manifest whose seq is above the manifest's snapshot (an inline snapshot of a
/// rotation that never committed, or a background snapshot never named). Non-empty unreferenced segments are a
/// corruption.
fn remove_unreferenced(dir: &Path, m: &Manifest) -> Result<Vec<String>, LedgerError> {
    let mut removed = Vec::new();
    // procfix7 round 5: a COMPLETE listing; each removal proved on the object itself just before it
    for e in crate::fsx::list_required("ledger.listing", dir).map_err(io_err(dir, "list"))? {
        let name = e.name_lossy();
        if let Some(idx) = segment_index(&name) {
            if idx > m.active.index {
                // proof: a regular file of length 0 (nothing of a history can be in it)
                match crate::fsx::presence_lstat("ledger.segment", &e.path) {
                    crate::fsx::Presence::Present(md) if md.is_file() && md.len() == 0 => {}
                    crate::fsx::Presence::Absent => continue,
                    crate::fsx::Presence::Present(_) => {
                        return Err(corrupt(&name, 0, "non-empty segment not in the manifest"));
                    }
                    crate::fsx::Presence::Unknown(err) => {
                        return Err(io_err(&e.path, "stat segment")(err));
                    }
                }
                fs::remove_file(&e.path).map_err(io_err(dir, "remove"))?;
                removed.push(name);
            }
        } else if let Some(sq) = name
            .strip_prefix("snapshot-")
            .and_then(|r| r.strip_suffix(".json"))
            .and_then(|n| n.parse::<u64>().ok())
            && m.snapshot.as_ref().is_none_or(|s| sq > s.seq)
            && snapshot_removable(&e.path, sq)
        {
            fs::remove_file(&e.path).map_err(io_err(dir, "remove"))?;
            removed.push(name);
        }
    }
    let _ = atomic::remove_stale_temporaries(dir);
    removed.sort();
    Ok(removed)
}

fn prune_snapshots(dir: &Path, m: &Manifest, keep_old: usize) {
    let Some(cur) = &m.snapshot else { return };
    // procfix7 round 5: only after a COMPLETE listing (an incomplete one prunes nothing), each file proved first
    let crate::fsx::Listing::Complete(entries) = crate::fsx::list("ledger.listing", dir) else {
        return;
    };
    let mut old: Vec<u64> = entries
        .iter()
        .filter_map(|e| {
            e.name
                .to_str()?
                .strip_prefix("snapshot-")?
                .strip_suffix(".json")?
                .parse::<u64>()
                .ok()
        })
        .filter(|&s| s < cur.seq)
        .collect();
    old.sort_unstable();
    let n = old.len().saturating_sub(keep_old);
    for s in &old[..n] {
        let p = dir.join(snapshot_name(*s));
        if snapshot_removable(&p, *s) {
            #[allow(clippy::let_underscore_must_use)]
            // ignored-ok (procfix7): pruning an old (proved) snapshot is best effort: kept files only cost space
            let _ = fs::remove_file(&p);
        }
    }
}

// ------------------------------------------------------------------------------------------------------------
// Full reading (verify, results, tools): read-only, streaming, verified exactly like `open`
// ------------------------------------------------------------------------------------------------------------

/// Result of reading the raw segments from record 1.
#[derive(Debug)]
pub struct ChainReport {
    pub records: u64,
    pub segments: u32,
    pub head: String,
    pub state: CampaignState,
    /// `Some(true)` if the manifest's snapshot equals the state replayed up to its seq (compared by digests),
    /// `None` if there is none to compare (no snapshot, its file gone, another snapshot version).
    pub snapshot_matches: Option<bool>,
    /// Bytes after the last complete record of the active segment (a torn tail not yet recovered).
    pub torn_tail_bytes: u64,
    pub mark: Option<CommitMark>,
}

/// Reads EVERY record from segment 1 (ignoring snapshots), verifying sealed digests, framing, canonical form,
/// sequence, the hash chain across segments and the commit mark, reducing as it goes; compares the snapshot.
/// Read-only: never repairs. Memory: one reduced state + one segment (the snapshot is compared by digests).
pub fn verify_chain(cdir: &Path) -> Result<ChainReport, LedgerError> {
    verify_chain_capture(cdir, None)
}

/// [`verify_chain`] that also captures the state and head right after record `cap.at` (one streaming pass,
/// memory = one segment + the state, + one copy-on-write clone of the state at `at`).
pub fn verify_chain_capture(
    cdir: &Path,
    mut cap: Option<&mut Capture>,
) -> Result<ChainReport, LedgerError> {
    let dir = ledger_dir(cdir);
    let mark = read_commit_mark(&dir)?;
    let m = read_manifest(&dir)?;
    let mut st = CampaignState::default();
    let (mut seq, mut head) = (0u64, GENESIS.to_string());
    let mut n = 0;
    let mut snapshot_matches = None;
    for s in &m.sealed {
        let (k, h) = replay_sealed(
            &dir,
            s,
            seq,
            &head,
            &mut reducer(&mut st, cap.as_deref_mut()),
        )?;
        n += k;
        seq = s.last_seq;
        head = h;
        if let Some(sr) = &m.snapshot
            && sr.seq == seq
        {
            snapshot_matches = snapshot_matches_state(&dir, sr, &head, &st)?;
        }
    }
    if seq + 1 != m.active.first_seq || head != m.active.prev {
        return Err(corrupt(
            MANIFEST_FILE,
            0,
            "active segment does not continue the chain",
        ));
    }
    let am = mark_for_active(&m, mark.as_ref())?;
    let bytes = read_segment_file(&dir, &m.active.file)?;
    let scan = scan_active(
        &m.active.file,
        &bytes,
        seq,
        head,
        am,
        None,
        &mut reducer(&mut st, cap),
    )?;
    if let Some((line, why)) = scan.corrupt {
        return Err(corrupt(&m.active.file, line, why));
    }
    Ok(ChainReport {
        records: n + scan.replayed,
        segments: m.sealed.len() as u32 + 1,
        head: scan.head,
        state: st,
        snapshot_matches,
        torn_tail_bytes: (bytes.len() - scan.pos) as u64,
        mark,
    })
}

/// Where a full read ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordsEnd {
    pub seq: u64,
    pub head: String,
    /// Bytes after the last record of the active segment (a torn tail beyond the commit mark), with the reason.
    pub torn_tail: Option<(u64, String)>,
}

/// Streams every committed record of the ledger, in order, with its chain value `h`, to `visit`, verified
/// EXACTLY like `open` (audit-ledger2 M3): sealed segments against the manifest (size, BLAKE3, last hash),
/// framing, digest, canonical form, dense `seq`, `prev` chain and the COMMIT MARK. Only a torn tail beyond the mark
/// ends the history silently (reported in [`RecordsEnd::torn_tail`]); a cut or altered acknowledged record, a
/// mark beyond the data or a missing segment is an error, never a shorter history. Memory: one segment.
pub fn for_each_record(
    cdir: &Path,
    visit: &mut dyn FnMut(&Record, &str) -> Result<(), LedgerError>,
) -> Result<RecordsEnd, LedgerError> {
    let dir = ledger_dir(cdir);
    if !manifest_present(&dir)? {
        return Err(LedgerError::NotALedger { dir });
    }
    let mark = read_commit_mark(&dir)?;
    let m = read_manifest(&dir)?;
    let (mut seq, mut head) = (0u64, GENESIS.to_string());
    for s in &m.sealed {
        let (_, h) = replay_sealed(&dir, s, seq, &head, visit)?;
        seq = s.last_seq;
        head = h;
    }
    if seq + 1 != m.active.first_seq || head != m.active.prev {
        return Err(corrupt(
            MANIFEST_FILE,
            0,
            "active segment does not continue the chain",
        ));
    }
    let am = mark_for_active(&m, mark.as_ref())?;
    let bytes = read_segment_file(&dir, &m.active.file)?;
    let scan = scan_active(&m.active.file, &bytes, seq, head, am, None, visit)?;
    if let Some((line, why)) = scan.corrupt {
        return Err(corrupt(&m.active.file, line, why));
    }
    Ok(RecordsEnd {
        seq: scan.seq,
        head: scan.head,
        torn_tail: scan.torn.map(|w| ((bytes.len() - scan.pos) as u64, w)),
    })
}

/// The reduced state of a FULL streaming replay from record 1 (no snapshot; verified like [`for_each_record`]):
/// results and audits that must not trust a snapshot (`supervisor::result::read_final_result`). Memory: one
/// reduced state + one segment.
pub fn replay_state(cdir: &Path) -> Result<(CampaignState, RecordsEnd), LedgerError> {
    let mut st = CampaignState::default();
    let end = for_each_record(cdir, &mut reducer(&mut st, None))?;
    Ok((st, end))
}

/// All committed records with their chain values `h` (in memory: for tools and tests; streaming users call
/// [`for_each_record`]). Verified like `open` (commit mark included).
pub fn read_records_with_hashes(cdir: &Path) -> Result<Vec<(Record, String)>, LedgerError> {
    let mut out = Vec::new();
    for_each_record(cdir, &mut |r, h| {
        out.push((r.clone(), h.to_string()));
        Ok(())
    })?;
    Ok(out)
}

/// All committed records in order (in memory; see [`read_records_with_hashes`]).
pub fn read_records(cdir: &Path) -> Result<Vec<Record>, LedgerError> {
    let mut out = Vec::new();
    for_each_record(cdir, &mut |r, _| {
        out.push(r.clone());
        Ok(())
    })?;
    Ok(out)
}

/// Content id helper: first 32 hex characters of BLAKE3 over the parts, each LENGTH-PREFIXED (u64 little
/// endian byte count, then the bytes), after a count prefix: unambiguous for any component, NUL included.
pub fn content_id(parts: &[&str]) -> String {
    let mut h = blake3::Hasher::new();
    h.update(b"rustred.campaign.content-id.v2\0");
    h.update(&(parts.len() as u64).to_le_bytes());
    for p in parts {
        h.update(&(p.len() as u64).to_le_bytes());
        h.update(p.as_bytes());
    }
    h.finalize().to_hex()[..32].to_string()
}
