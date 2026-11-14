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
use std::sync::atomic::{AtomicU64, Ordering};
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
pub const SNAPSHOT_SCHEMA: &str = "rustred.campaign.snapshot.v1";
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
    /// WRITE-AHEAD intent: fsynced before the child is spawned.
    Launch {
        run_dir: String,
        cpus: Vec<u32>,
        reserved_bytes: u64,
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
    EvidenceImported {
        evidence: String,
        producer: String,
        description: String,
        files: Vec<FileRef>,
        level_claimed: EvidenceLevel,
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

/// Reads the commit mark of a ledger directory (`None`: no valid slot).
pub fn read_commit_mark(dir: &Path) -> Result<Option<CommitMark>, LedgerError> {
    let p = dir.join(COMMIT_FILE);
    match fs::read(&p) {
        Ok(b) => Ok(CommitMark::read(&b)),
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
}

impl Default for LedgerConfig {
    fn default() -> Self {
        LedgerConfig {
            rotate_bytes: 64 << 20,
            lazy_max_records: 256,
            lazy_max_ms: 1000,
            keep_old_snapshots: 1,
            rotation_retry_ms: 10_000,
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

/// Result of scanning the active segment.
struct Scan {
    replayed: u64,
    /// End of the last good record.
    pos: usize,
    seq: u64,
    head: String,
    hasher: blake3::Hasher,
    /// A torn tail (beyond the mark, incomplete or NUL-filled, nothing valid after it).
    torn: Option<String>,
    /// Corruption found (only returned when an override may handle it; else an error).
    corrupt: Option<(u64, String)>,
}

fn is_torn_shape(line_incomplete: bool, bytes: &[u8]) -> bool {
    line_incomplete || bytes.contains(&0)
}

/// Scans `bytes` (the active segment) on top of `state` (at `seq`/`head`). `mark` = the commit mark if it names
/// this segment; `stop_after` = stop after this seq (override). Reducer refusals are errors (`Replay`).
fn scan_active(
    file: &str,
    bytes: &[u8],
    state: &mut CampaignState,
    mut seq: u64,
    mut head: String,
    mark: Option<&CommitMark>,
    stop_after: Option<u64>,
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
                fail = Some((
                    format!("line {line_no}: {why}"),
                    is_torn_shape(false, line),
                ));
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
                format!("line {line_no}: hash chain broken (prev does not match the previous record)"),
                false,
            ));
            break;
        }
        state.apply_record(&rec).map_err(LedgerError::Replay)?;
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
        let mut is_torn = torn_shape && (pos as u64) >= mark_off;
        if !torn_shape {
            why.push_str(" (a complete line: corruption, not a torn write)");
        } else if (pos as u64) < mark_off {
            why.push_str(&format!(
                " (at or before the commit mark at offset {mark_off}: acknowledged data damaged)"
            ));
        }
        if is_torn {
            // Refuse if any later complete line is a valid record: that is not a torn tail.
            let rest = &bytes[pos..];
            if let Some(first_nl) = rest.iter().position(|&b| b == b'\n') {
                let mut p = first_nl + 1;
                while let Some(nl) = rest[p..].iter().position(|&b| b == b'\n') {
                    if decode_record(&rest[p..p + nl]).is_ok() {
                        is_torn = false;
                        why.push_str("; valid records follow");
                        break;
                    }
                    p += nl + 1;
                }
            }
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

fn read_snapshot_checked(dir: &Path, s: &SnapshotRef) -> Result<Snapshot, LedgerError> {
    let p = dir.join(&s.file);
    let bytes = atomic::read_capped(&p, 16 << 30)?;
    if blake3::hash(&bytes).to_hex().as_str() != s.blake3 {
        return Err(corrupt(&s.file, 0, "snapshot digest differs from the manifest"));
    }
    let snap: Snapshot =
        serde_json::from_slice(&bytes).map_err(|e| corrupt(&s.file, 0, e.to_string()))?;
    if snap.schema != SNAPSHOT_SCHEMA || snap.seq != s.seq {
        return Err(corrupt(&s.file, 0, "snapshot schema or seq mismatch"));
    }
    Ok(snap)
}

/// Replays one sealed segment into `st`, checking its digest, framing, sequence and chain.
fn replay_sealed(
    dir: &Path,
    s: &SealedSegment,
    st: &mut CampaignState,
    mut seq: u64,
    head: &str,
) -> Result<(u64, String), LedgerError> {
    let p = dir.join(&s.file);
    let bytes = fs::read(&p).map_err(io_err(&p, "read sealed segment"))?;
    if blake3::hash(&bytes).to_hex().as_str() != s.blake3 {
        return Err(corrupt(
            &s.file,
            0,
            "sealed segment digest differs from the manifest (edited or truncated)",
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
        st.apply_record(&rec).map_err(LedgerError::Replay)?;
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

/// State at the start of the active segment: snapshot (with a length check of every sealed segment and a digest
/// check of the last one, audit A M2) or replay of all sealed segments.
fn load_base(
    dir: &Path,
    m: &Manifest,
) -> Result<(CampaignState, u64, String, Option<u64>, u64), LedgerError> {
    let (state, seq, head, from_snapshot, replayed) = match &m.snapshot {
        Some(s) => {
            for sealed in &m.sealed {
                let p = dir.join(&sealed.file);
                let len = fs::metadata(&p)
                    .map_err(io_err(&p, "stat sealed segment"))?
                    .len();
                if len != sealed.bytes {
                    return Err(corrupt(
                        &sealed.file,
                        0,
                        format!(
                            "sealed segment has {len} bytes, the manifest says {}",
                            sealed.bytes
                        ),
                    ));
                }
            }
            if let Some(last) = m.sealed.last() {
                let p = dir.join(&last.file);
                let (_, d) = atomic::blake3_file(&p).map_err(io_err(&p, "digest sealed segment"))?;
                if d != last.blake3 {
                    return Err(corrupt(
                        &last.file,
                        0,
                        "last sealed segment digest differs from the manifest",
                    ));
                }
            }
            let snap = read_snapshot_checked(dir, s)?;
            (snap.state, snap.seq, snap.head, Some(s.seq), 0)
        }
        None => {
            let mut st = CampaignState::default();
            let mut seq = 0;
            let mut head = GENESIS.to_string();
            let mut n = 0;
            for s in &m.sealed {
                let (k, h) = replay_sealed(dir, s, &mut st, seq, &head)?;
                n += k;
                seq = s.last_seq;
                head = h;
            }
            (st, seq, head, None, n)
        }
    };
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
    Ok((state, seq, head, from_snapshot, replayed))
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
/// cannot make the mark look ahead of the data). A torn tail is reported, corruption is an error.
pub fn open_read_only(cdir: &Path) -> Result<LedgerView, LedgerError> {
    let dir = ledger_dir(cdir);
    if !dir.join(MANIFEST_FILE).exists() {
        return Err(LedgerError::NotALedger { dir });
    }
    let mark = read_commit_mark(&dir)?;
    let manifest = read_manifest(&dir)?;
    let (mut state, seq, head, _, _) = load_base(&dir, &manifest)?;
    let am = mark_for_active(&manifest, mark.as_ref())?;
    let seg_path = dir.join(&manifest.active.file);
    let bytes = fs::read(&seg_path).map_err(io_err(&seg_path, "read segment"))?;
    let scan = scan_active(
        &manifest.active.file,
        &bytes,
        &mut state,
        seq,
        head,
        am,
        None,
    )?;
    if let Some((line, why)) = scan.corrupt {
        return Err(corrupt(&manifest.active.file, line, why));
    }
    Ok(LedgerView {
        state,
        seq: scan.seq,
        head: scan.head,
        unverified_tail: scan
            .torn
            .map(|why| ((bytes.len() - scan.pos) as u64, why)),
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
    poisoned: Option<String>,
    warnings: Vec<String>,
    rotation_failed_at: Option<Duration>,
    _dir_lock: File,
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

fn check_lock(cdir: &Path, lock: &CampaignLock) -> Result<(), LedgerError> {
    let canon = |p: &Path| fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if canon(lock.dir()) != canon(cdir) {
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

#[derive(Serialize, Deserialize)]
struct PendingRecovery {
    info: RecoveryInfo,
    truncate_to: u64,
}

impl Ledger {
    /// Creates a new ledger in `cdir/ledger` and writes record 1 (`campaign_created`). Record 1 is written and
    /// fsynced BEFORE the manifest is installed (the commit point), so a crash leaves either no ledger (create
    /// again works: leftovers are removed) or a ledger with record 1 (audit A m4).
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
        if dir.join(MANIFEST_FILE).exists() {
            return Err(LedgerError::AlreadyExists { dir });
        }
        let fresh = !dir.exists();
        fs::create_dir_all(&dir).map_err(io_err(&dir, "create dir"))?;
        if fresh {
            ops.sync_dir(cdir).map_err(io_err(cdir, "sync dir"))?;
        }
        let dir_lock = flock_dir(&dir)?;
        // Leftovers of a crashed create (no manifest = no ledger).
        for e in fs::read_dir(&dir).map_err(io_err(&dir, "list"))?.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name == COMMIT_FILE
                || name == RECOVERY_PENDING_FILE
                || (name.starts_with("segment-") && name.ends_with(".jsonl"))
                || (name.starts_with("snapshot-") && name.ends_with(".json"))
            {
                ops.remove(&e.path()).map_err(io_err(&dir, "remove leftover"))?;
            }
        }
        let _ = atomic::remove_stale_temporaries(&dir);
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
            _dir_lock: dir_lock,
        };
        l.append(Event::CampaignCreated {
            campaign_id: genesis.campaign_id,
            spec_blake3: genesis.spec_blake3,
            terminal_bound: genesis.terminal_bound,
            mode: genesis.mode,
        })?;
        if let Some(w) = l.warnings.first() {
            return Err(LedgerError::Poisoned { why: w.clone() });
        }
        l.ops
            .sync_dir(&dir)
            .map_err(io_err(&dir, "sync dir"))?;
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
        if !dir.join(MANIFEST_FILE).exists() {
            return Err(LedgerError::NotALedger { dir });
        }
        let dir_lock = flock_dir(&dir)?;
        let manifest = read_manifest(&dir)?;
        let mut report = OpenReport {
            removed_unreferenced: remove_unreferenced(&dir, &manifest)?,
            ..OpenReport::default()
        };
        let mark = read_commit_mark(&dir)?;
        let pending: Option<PendingRecovery> = match fs::read(dir.join(RECOVERY_PENDING_FILE)) {
            Ok(b) => Some(
                serde_json::from_slice(&b)
                    .map_err(|e| corrupt(RECOVERY_PENDING_FILE, 0, e.to_string()))?,
            ),
            Err(_) => None,
        };
        let (mut state, base_seq, base_head, from_snapshot, replayed) =
            load_base(&dir, &manifest)?;
        report.from_snapshot = from_snapshot;
        report.replayed = replayed;
        let am = mark_for_active(&manifest, mark.as_ref())?.cloned();
        let seg_path = dir.join(&manifest.active.file);
        let bytes = fs::read(&seg_path).map_err(io_err(&seg_path, "read segment"))?;
        let base_state = state.clone();
        let mut scan = scan_active(
            &manifest.active.file,
            &bytes,
            &mut state,
            base_seq,
            base_head.clone(),
            am.as_ref(),
            None,
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
            state = base_state;
            scan = scan_active(
                &manifest.active.file,
                &bytes,
                &mut state,
                base_seq,
                base_head,
                None,
                Some(to),
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
            _dir_lock: dir_lock,
        };
        report.replayed += scan.replayed;
        // Truncation (torn tail or override): marker first (crash window, audit A m5), dropped bytes saved.
        if let Some((mut info, truncate_to)) = recovery {
            let dropped = &bytes[truncate_to as usize..];
            let saved = match &pending {
                Some(p) if p.truncate_to == truncate_to => p.info.saved_as.clone(),
                _ => {
                    let name = format!("torn-{:012}-{}.bin", info.last_good_seq, l.clock.now_utc().0);
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
            l.finish_recovery(&info)?;
            report.recovery = Some(info);
        } else if let Some(p) = pending {
            // The truncation happened before a crash; its record may be missing.
            if !l.state.recoveries.contains(&p.info.saved_as.clone().unwrap_or_default()) {
                l.finish_recovery(&p.info)?;
            } else {
                let _ = l.ops.remove(&dir.join(RECOVERY_PENDING_FILE));
            }
            report.recovery = Some(p.info);
        }
        // Fsync gate: rewrite and fsync everything beyond the commit mark, then advance the mark.
        let mark_off = match &am {
            Some(m) => m.offset,
            None => 0,
        };
        if l.active_len > mark_off && l.poisoned.is_none() {
            let cur = fs::read(&seg_path).map_err(io_err(&seg_path, "re-read segment"))?;
            let end = (l.active_len as usize).min(cur.len());
            let tail = &cur[mark_off as usize..end];
            l.file
                .write_at(mark_off, tail)
                .map_err(io_err(&seg_path, "fsync gate rewrite"))?;
            l.file
                .sync()
                .map_err(io_err(&seg_path, "fsync gate"))?;
            report.regated_bytes = tail.len() as u64;
            l.write_mark();
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
    /// Non-fatal conditions after acknowledged appends (failed rotation, failed commit-mark write).
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    fn poisoned_err(&self) -> Option<LedgerError> {
        self.poisoned
            .as_ref()
            .map(|why| LedgerError::Poisoned { why: why.clone() })
    }

    /// Appends one record (validated by the reducer first: an illegal record is never written). Returns its seq.
    pub fn append(&mut self, event: Event) -> Result<u64, LedgerError> {
        let s = self.append_all(vec![event])?;
        Ok(s[0])
    }

    /// Appends several records with ONE fsync at the end if any of them is `Sync` (group commit). All or nothing
    /// against the reducer. `Ok` = every record is written (and durable if any is `Sync`); a later rotation
    /// failure is a warning, never an error of this call (audit A M2).
    pub fn append_all(&mut self, events: Vec<Event>) -> Result<Vec<u64>, LedgerError> {
        if let Some(e) = self.poisoned_err() {
            return Err(e);
        }
        let start_len = self.active_len;
        let mut seqs = Vec::with_capacity(events.len());
        let mut need_sync = false;
        let mut buf = Vec::new();
        let saved_state = if events.len() > 1 {
            Some(self.state.clone())
        } else {
            None
        };
        let restore = |l: &mut Ledger, s: Option<CampaignState>| {
            if let Some(s) = s {
                l.state = s;
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
                restore(self, saved_state);
                return Err(LedgerError::TooLarge { bytes: line.len() });
            }
            if let Err(e) = self.state.apply_record(&rec) {
                restore(self, saved_state);
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
            self.fail_write(start_len, saved_state, &err);
            return Err(err);
        }
        let n = seqs.len();
        let now = self.clock.monotonic();
        let due = self.unsynced + n >= self.cfg.lazy_max_records
            || now.saturating_sub(self.last_sync) >= Duration::from_millis(self.cfg.lazy_max_ms);
        if need_sync || due {
            if let Err(e) = self.file.sync() {
                let err = io_err(&path, "fsync")(e);
                self.fail_write(start_len, saved_state, &err);
                return Err(err);
            }
        }
        self.active_len += buf.len() as u64;
        self.active_hasher.update(&buf);
        self.seq = seq;
        self.head = head;
        if need_sync || due {
            self.unsynced = 0;
            self.last_sync = now;
            self.write_mark();
        } else {
            self.unsynced += n;
        }
        if self.active_len >= self.cfg.rotate_bytes {
            let retry_ok = self.rotation_failed_at.is_none_or(|t| {
                now.saturating_sub(t) >= Duration::from_millis(self.cfg.rotation_retry_ms)
            });
            if retry_ok && let Err(e) = self.rotate() {
                self.rotation_failed_at = Some(now);
                self.warnings
                    .push(format!("rotation failed after record {seq} (retried later): {e}"));
            }
        }
        Ok(seqs)
    }

    /// A failed write / fsync: truncate the batch away (best effort), poison, rebuild the state from disk.
    fn fail_write(&mut self, start_len: u64, saved: Option<CampaignState>, err: &LedgerError) {
        let truncated = self.file.truncate(start_len).is_ok();
        self.poisoned = Some(format!(
            "{err}; batch {}",
            if truncated {
                "truncated away"
            } else {
                "may remain on disk"
            }
        ));
        if let Some(s) = saved {
            self.state = s;
            return;
        }
        match self.rebuild_state(start_len) {
            Ok(st) => self.state = st,
            Err(e) => {
                self.poisoned = Some(format!(
                    "{}; state rebuild failed too: {e}",
                    self.poisoned.take().unwrap_or_default()
                ))
            }
        }
    }

    /// The state of the records in the active segment's first `len` bytes (from disk).
    fn rebuild_state(&self, len: u64) -> Result<CampaignState, LedgerError> {
        let (mut st, seq, head, _, _) = load_base(&self.dir, &self.manifest)?;
        let p = self.dir.join(&self.manifest.active.file);
        let bytes = fs::read(&p).map_err(io_err(&p, "read segment"))?;
        let cut = (len as usize).min(bytes.len());
        scan_active(
            &self.manifest.active.file,
            &bytes[..cut],
            &mut st,
            seq,
            head,
            None,
            None,
        )?;
        Ok(st)
    }

    /// Writes and fsyncs the commit mark at the current end (after a successful segment fsync). A failure is a
    /// warning: the records are durable; the next mark write rewrites the whole slot.
    fn write_mark(&mut self) {
        self.mark_counter += 1;
        let m = CommitMark {
            counter: self.mark_counter,
            segment: self.manifest.active.index,
            seq: self.seq,
            offset: self.active_len,
            head: self.head.clone(),
        };
        let slot = (self.mark_counter % 2) as u64 * MARK_SLOT_BYTES as u64;
        let r = self
            .mark_file
            .write_at(slot, &m.encode())
            .and_then(|_| self.mark_file.sync());
        if let Err(e) = r {
            self.warnings
                .push(format!("commit mark write failed at seq {}: {e}", self.seq));
        }
    }

    /// fsync of the active segment (makes lazy records durable), then the commit mark. A failure poisons.
    pub fn sync(&mut self) -> Result<(), LedgerError> {
        if let Some(e) = self.poisoned_err() {
            return Err(e);
        }
        let path = self.dir.join(&self.manifest.active.file);
        if let Err(e) = self.file.sync() {
            let err = io_err(&path, "fsync")(e);
            self.poisoned = Some(format!("{err}"));
            return Err(err);
        }
        self.unsynced = 0;
        self.last_sync = self.clock.monotonic();
        self.write_mark();
        Ok(())
    }

    /// Seals the active segment, writes a snapshot of the reduced state, starts a new segment. Steps (the
    /// manifest rename is the commit point; a crash before it leaves unreferenced files that `open` removes):
    /// (1) fsync active + mark; (2) snapshot file (atomic); (3) new empty segment + dir fsync; (4) manifest
    /// (atomic); (5) old segment chmod 0444; (6) `segment_opened` record in the new segment; (7) prune snapshots.
    /// An error before the manifest rename leaves the ledger as it was (the caller may retry); an error after it
    /// poisons the handle.
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
        let snap = Snapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            seq: self.seq,
            head: self.head.clone(),
            state: self.state.clone(),
        };
        let snap_name = snapshot_name(self.seq);
        let mut snap_bytes = serde_json::to_vec(&snap).expect("snapshot serialises");
        snap_bytes.push(b'\n');
        let snap_path = self.dir.join(&snap_name);
        install(self.ops.as_ref(), &snap_path, &snap_bytes)
            .map_err(|f| install_err(&snap_path, f))?;
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
        m.snapshot = Some(SnapshotRef {
            file: snap_name,
            seq: self.seq,
            blake3: blake3::hash(&snap_bytes).to_hex().to_string(),
            state_digest: self.state.digest(),
        });
        let mpath = self.dir.join(MANIFEST_FILE);
        match install(self.ops.as_ref(), &mpath, &manifest_bytes(&m)) {
            Ok(()) => {}
            Err(InstallFail::BeforeRename(e)) => return Err(io_err(&mpath, "install manifest")(e)),
            Err(InstallFail::AfterRename(e)) => {
                let err = io_err(&mpath, "sync dir after manifest rename")(e);
                self.poisoned = Some(format!(
                    "{err}: the on-disk manifest may name the new segment"
                ));
                return Err(err);
            }
        }
        self.manifest = m;
        let _ = fs::set_permissions(self.dir.join(&old.file), fs::Permissions::from_mode(0o444));
        self.file = new_file;
        self.active_len = 0;
        self.active_hasher = blake3::Hasher::new();
        let snapshot_seq = self.seq;
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
    if !p.exists() {
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
/// and snapshot files not in the manifest whose seq is above the manifest's snapshot. Non-empty unreferenced
/// segments are a corruption.
fn remove_unreferenced(dir: &Path, m: &Manifest) -> Result<Vec<String>, LedgerError> {
    let mut removed = Vec::new();
    let rd = fs::read_dir(dir).map_err(io_err(dir, "list"))?;
    for e in rd {
        let e = e.map_err(io_err(dir, "list"))?;
        let name = e.file_name().to_string_lossy().into_owned();
        if let Some(idx) = name
            .strip_prefix("segment-")
            .and_then(|r| r.strip_suffix(".jsonl"))
            .and_then(|n| n.parse::<u32>().ok())
        {
            if idx > m.active.index {
                let len = e.metadata().map(|md| md.len()).unwrap_or(1);
                if len != 0 {
                    return Err(corrupt(&name, 0, "non-empty segment not in the manifest"));
                }
                fs::remove_file(e.path()).map_err(io_err(dir, "remove"))?;
                removed.push(name);
            }
        } else if let Some(sq) = name
            .strip_prefix("snapshot-")
            .and_then(|r| r.strip_suffix(".json"))
            .and_then(|n| n.parse::<u64>().ok())
            && m.snapshot.as_ref().is_none_or(|s| sq > s.seq)
        {
            fs::remove_file(e.path()).map_err(io_err(dir, "remove"))?;
            removed.push(name);
        }
    }
    let _ = atomic::remove_stale_temporaries(dir);
    removed.sort();
    Ok(removed)
}

fn prune_snapshots(dir: &Path, m: &Manifest, keep_old: usize) {
    let Some(cur) = &m.snapshot else { return };
    let Ok(rd) = fs::read_dir(dir) else { return };
    let mut old: Vec<u64> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            e.file_name()
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
        let _ = fs::remove_file(dir.join(snapshot_name(*s)));
    }
}

// ------------------------------------------------------------------------------------------------------------
// Full reading (verify, tests, tools): read-only
// ------------------------------------------------------------------------------------------------------------

/// Result of reading the raw segments from record 1.
#[derive(Debug)]
pub struct ChainReport {
    pub records: u64,
    pub segments: u32,
    pub head: String,
    pub state: CampaignState,
    /// `Some(true)` if the manifest's snapshot equals the state replayed up to its seq.
    pub snapshot_matches: Option<bool>,
    /// Bytes after the last complete record of the active segment (a torn tail not yet recovered).
    pub torn_tail_bytes: u64,
    pub mark: Option<CommitMark>,
}

/// Reads EVERY record from segment 1 (ignoring snapshots), verifying sealed digests, framing, canonical form,
/// sequence, the hash chain across segments and the commit mark, reducing as it goes; compares the snapshot.
/// Read-only: never repairs.
pub fn verify_chain(cdir: &Path) -> Result<ChainReport, LedgerError> {
    let dir = ledger_dir(cdir);
    let mark = read_commit_mark(&dir)?;
    let m = read_manifest(&dir)?;
    let mut st = CampaignState::default();
    let (mut seq, mut head) = (0u64, GENESIS.to_string());
    let mut n = 0;
    let mut snapshot_matches = None;
    for s in &m.sealed {
        let (k, h) = replay_sealed(&dir, s, &mut st, seq, &head)?;
        n += k;
        seq = s.last_seq;
        head = h;
        if let Some(sr) = &m.snapshot
            && sr.seq == seq
        {
            let snap = read_snapshot_checked(&dir, sr)?;
            snapshot_matches =
                Some(snap.state == st && snap.head == head && sr.state_digest == st.digest());
        }
    }
    if seq + 1 != m.active.first_seq || head != m.active.prev {
        return Err(corrupt(MANIFEST_FILE, 0, "active segment does not continue the chain"));
    }
    let am = mark_for_active(&m, mark.as_ref())?;
    let p = dir.join(&m.active.file);
    let bytes = fs::read(&p).map_err(io_err(&p, "read"))?;
    let scan = scan_active(&m.active.file, &bytes, &mut st, seq, head, am, None)?;
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

/// All complete, valid records of the ledger in order with their chain values `h` (no reduction; for tools and
/// tests). Stops at the first line that does not decode in the active segment (a torn tail); a bad line in a
/// sealed segment is an error.
pub fn read_records_with_hashes(cdir: &Path) -> Result<Vec<(Record, String)>, LedgerError> {
    let dir = ledger_dir(cdir);
    let m = read_manifest(&dir)?;
    let mut out = Vec::new();
    let n_sealed = m.sealed.len();
    let files: Vec<String> = m
        .sealed
        .iter()
        .map(|s| s.file.clone())
        .chain(std::iter::once(m.active.file.clone()))
        .collect();
    for (i, f) in files.iter().enumerate() {
        let p = dir.join(f);
        let bytes = fs::read(&p).map_err(io_err(&p, "read"))?;
        let mut pos = 0;
        let mut line = 0;
        while let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') {
            line += 1;
            match decode_record(&bytes[pos..pos + nl]) {
                Ok(x) => out.push(x),
                Err(_) if i == n_sealed => break,
                Err(why) => return Err(corrupt(f, line, why)),
            }
            pos += nl + 1;
        }
    }
    Ok(out)
}

/// All complete, valid records of the ledger in order.
pub fn read_records(cdir: &Path) -> Result<Vec<Record>, LedgerError> {
    Ok(read_records_with_hashes(cdir)?
        .into_iter()
        .map(|(r, _)| r)
        .collect())
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
