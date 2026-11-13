//! The LEDGER OF RECORD of a campaign: append-only JSONL, schema `rustred.campaign.ledger.v1`, one record per
//! state change, hash-chained, segmented with a manifest and snapshots.
//!
//! Layout (inside the campaign directory CDIR):
//! ```text
//! CDIR/ledger/MANIFEST.json            rustred.campaign.ledger-manifest.v1 (atomic; the commit point of rotation)
//! CDIR/ledger/segment-000001.jsonl     sealed segments: read-only (0444), BLAKE3 + last hash in the manifest
//! CDIR/ledger/segment-000002.jsonl     the active segment (append-only)
//! CDIR/ledger/snapshot-000000004711.json  rustred.campaign.snapshot.v1: reduced state after record 4711
//! CDIR/ledger/torn-<seq>-<utc ms>.bin  bytes dropped by a torn-tail recovery (kept for forensics)
//! ```
//!
//! Record line (canonical bytes = the line exactly as written, without the newline; the writer serialises
//! structs with a fixed field order and no floats, so the bytes are deterministic):
//! `{"schema":"rustred.campaign.ledger.v1","seq":N,"utc":"...Z","prev":"<hex blake3 of the previous line>",
//!   "event":{"kind":"...","body":{...}}}` ; `prev` of record 1 is 64 zeros.
//!
//! FSYNC POLICY ([`Event::durability`]): `Sync` records are fsynced (`sync_data`) before `append` returns: every
//! record that moves something into an accepted or terminal state, every write-ahead intent (job launch,
//! `intent`), every finished / interrupted / lost attempt, campaign phase changes, inputs, evidence, lock
//! takeovers, recoveries. `Lazy` records (item / job creation, item start, job progress, notes) are written at
//! once (one `write` per line, so they survive a process kill) but fsynced only with the next `Sync` record,
//! after `lazy_max_records` lazy records, or after `lazy_max_ms` of monotonic time, or by [`Ledger::sync`].
//! Because the file is append-only, an fsync of a later record makes every earlier record durable.
//!
//! Recovery on open: see [`Ledger::open`] and the interface document (torn tail truncated and saved, a
//! `recovery` record appended; corruption followed by valid records refuses).

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::atomic::{self, AtomicError};
use crate::clock::Clock;
use crate::lock::{DeadReason, OwnerRecord, ProcessIdentity};
use crate::state::{CampaignEvent, CampaignState, EvidenceLevel, FinishClass, NodeEvent, ReduceError, Rung};

pub const LEDGER_SCHEMA: &str = "rustred.campaign.ledger.v1";
pub const MANIFEST_SCHEMA: &str = "rustred.campaign.ledger-manifest.v1";
pub const SNAPSHOT_SCHEMA: &str = "rustred.campaign.snapshot.v1";
pub const GENESIS: &str = "0000000000000000000000000000000000000000000000000000000000000000";
pub const LEDGER_DIR: &str = "ledger";
pub const MANIFEST_FILE: &str = "MANIFEST.json";
/// Upper bound of one record line (large payloads go to files referenced by digest).
pub const MAX_RECORD_BYTES: usize = 64 * 1024;

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
    Job { job: String, attempt: u32, table_blake3: String },
    Evidence { evidence: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum ItemEvent {
    Start { job: Option<String> },
    /// The attempt's sealed table holds a candidate rule for this item.
    Produce { job: String, attempt: u32, table: FileRef },
    /// `record` = the accepted rule record, written (atomically, content-addressed) BEFORE this line.
    Accept { rule: String, level: EvidenceLevel, record: FileRef, source: AcceptSource, elided: bool },
    /// Higher evidence for the accepted rule (identity test / proof); `evidence` names its certificate file.
    Upgrade { level: EvidenceLevel, evidence: FileRef },
    Reject { reason: String },
    Escalate { to: Rung, reason: String },
    Requeue { reason: String },
    ProposeTerminal { reason: String },
    /// `count` keys become terminals (counted against the node's fallback bound); `keys` = the key-set file.
    DeclareTerminal { count: u32, keys: Option<FileRef>, reason: String },
    MarkStale { input: String },
    Reopen { reason: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "ev", rename_all = "snake_case")]
pub enum JobEvent {
    /// WRITE-AHEAD intent: fsynced before the child is spawned.
    Launch { run_dir: String, cpus: Vec<u32>, reserved_bytes: u64 },
    Started { pid: u32, start_ticks: u64, boot_id: String },
    Finished {
        class: FinishClass,
        exit_code: Option<i32>,
        signal: Option<i32>,
        /// The sealed output (table; for checkers the verdict file) with its digest. Required for success kinds.
        sealed_output: Option<FileRef>,
        outcome: Option<FileRef>,
        cpu_ms: u64,
        wall_ms: u64,
        peak_rss_bytes: u64,
        time_decided: bool,
    },
    Interrupt { reason: String },
    Lose { reason: String },
    Requeue { reason: String },
    Retry { reason: String },
    Cancel { reason: String },
}

/// The closed list of record kinds of `rustred.campaign.ledger.v1` (additions = minor version, readers of an
/// older minor refuse unknown kinds: the ledger is never read tolerantly).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "body", rename_all = "snake_case")]
pub enum Event {
    CampaignCreated { campaign_id: String, spec_blake3: String, terminal_bound: u32 },
    Campaign { ev: CampaignEvent, reason: String },
    SupervisorStarted { identity: ProcessIdentity, argv: Vec<String>, version: String, lock_token: String },
    LockTakeover { previous: OwnerRecord, reason: DeadReason, orphans: Vec<u32> },
    Recovery { segment: u32, last_good_seq: u64, truncated_bytes: u64, saved_as: Option<String>, detail: String },
    SegmentOpened { index: u32, previous_blake3: String, snapshot_seq: u64 },
    InputRegistered { input: String, role: String, path: String, bytes: u64, blake3: String },
    EvidenceImported { evidence: String, producer: String, description: String, files: Vec<FileRef> },
    NodeRegistered { node: String, inputs: Vec<String>, terminal_bound: Option<u32> },
    Node { node: String, ev: NodeEvent },
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
    Item { item: String, ev: ItemEvent },
    JobQueued { job: String, item: String, rung: Rung, kind: String, spec_blake3: String, binary: String, reserved_bytes: u64 },
    Job { job: String, attempt: u32, ev: JobEvent },
    /// Write-ahead intent of a side effect (`action` e.g. `write_accepted_record`, `write_keyset`, `kill_orphan`).
    Intent { op: String, action: String, target: String },
    Done { op: String, result: Option<FileRef> },
    JobProgress { job: String, attempt: u32, counters: BTreeMap<String, u64> },
    Note { text: String },
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
            Event::NodeRegistered { .. } => "node_registered",
            Event::Node { .. } => "node",
            Event::ItemCreated { .. } => "item_created",
            Event::Item { .. } => "item",
            Event::JobQueued { .. } => "job_queued",
            Event::Job { .. } => "job",
            Event::Intent { .. } => "intent",
            Event::Done { .. } => "done",
            Event::JobProgress { .. } => "job_progress",
            Event::Note { .. } => "note",
        }
    }

    /// The fsync class (module doc).
    pub fn durability(&self) -> Durability {
        match self {
            Event::ItemCreated { .. }
            | Event::JobQueued { .. }
            | Event::Item { ev: ItemEvent::Start { .. }, .. }
            | Event::JobProgress { .. }
            | Event::Note { .. } => Durability::Lazy,
            _ => Durability::Sync,
        }
    }
}

/// One ledger line.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub seq: u64,
    pub utc: String,
    pub prev: String,
    pub event: Event,
}

/// blake3 hex of a line's canonical bytes.
pub fn line_hash(line: &[u8]) -> String {
    blake3::hash(line).to_hex().to_string()
}

// ------------------------------------------------------------------------------------------------------------
// Manifest and snapshot
// ------------------------------------------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedSegment {
    pub index: u32,
    pub file: String,
    pub first_seq: u64,
    pub last_seq: u64,
    pub bytes: u64,
    pub blake3: String,
    /// Hash of its last record (= `prev` of the next segment's first record).
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

fn segment_name(index: u32) -> String {
    format!("segment-{index:06}.jsonl")
}

fn snapshot_name(seq: u64) -> String {
    format!("snapshot-{seq:012}.json")
}

// ------------------------------------------------------------------------------------------------------------
// Segment file abstraction (fault injection for disk-full tests)
// ------------------------------------------------------------------------------------------------------------

pub trait SegmentFile: Send {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn sync(&mut self) -> io::Result<()>;
    fn truncate(&mut self, len: u64) -> io::Result<()>;
}

pub trait SegmentOpener: Send + Sync {
    fn open(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>>;
}

struct RealSegment(File);

impl SegmentFile for RealSegment {
    fn append(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.0.write_all(bytes)
    }
    fn sync(&mut self) -> io::Result<()> {
        self.0.sync_data()
    }
    fn truncate(&mut self, len: u64) -> io::Result<()> {
        self.0.set_len(len)?;
        self.0.sync_all()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RealOpener;

impl SegmentOpener for RealOpener {
    fn open(&self, path: &Path) -> io::Result<Box<dyn SegmentFile>> {
        Ok(Box::new(RealSegment(OpenOptions::new().append(true).create(true).open(path)?)))
    }
}

// ------------------------------------------------------------------------------------------------------------
// Errors and configuration
// ------------------------------------------------------------------------------------------------------------

#[derive(Debug)]
pub enum LedgerError {
    /// ENOSPC / EDQUOT: nothing of the failed record remains on disk; the handle is poisoned (reopen after
    /// freeing space).
    NoSpace { path: PathBuf },
    Io { path: PathBuf, op: &'static str, source: io::Error },
    Atomic(AtomicError),
    /// The record was refused by the reducer and NOT written.
    Refused(ReduceError),
    /// A stored record is refused by the reducer (code/ledger mismatch or tampering).
    Replay(ReduceError),
    /// Hash chain / sequence / digest / schema violation that is not a torn tail.
    Corrupt { file: String, line: u64, why: String },
    NotALedger { dir: PathBuf },
    AlreadyExists { dir: PathBuf },
    TooLarge { bytes: usize },
    /// A previous write failed; the in-memory state may be ahead of the disk.
    Poisoned,
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LedgerError::NoSpace { path } => write!(f, "disk full appending to {}; ledger intact, reopen after freeing space", path.display()),
            LedgerError::Io { path, op, source } => write!(f, "ledger I/O ({op}) on {}: {source}", path.display()),
            LedgerError::Atomic(e) => write!(f, "{e}"),
            LedgerError::Refused(e) => write!(f, "record refused (not written): {e}"),
            LedgerError::Replay(e) => write!(f, "stored record refused on replay: {e}"),
            LedgerError::Corrupt { file, line, why } => write!(f, "ledger corrupt at {file}:{line}: {why}"),
            LedgerError::NotALedger { dir } => write!(f, "{} holds no ledger", dir.display()),
            LedgerError::AlreadyExists { dir } => write!(f, "{} already holds a ledger", dir.display()),
            LedgerError::TooLarge { bytes } => write!(f, "record of {bytes} bytes exceeds {MAX_RECORD_BYTES}"),
            LedgerError::Poisoned => write!(f, "ledger handle poisoned by an earlier write failure; reopen"),
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
            LedgerError::NoSpace { path: path.to_path_buf() }
        } else {
            LedgerError::Io { path: path.to_path_buf(), op, source: e }
        }
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
}

impl Default for LedgerConfig {
    fn default() -> Self {
        LedgerConfig { rotate_bytes: 64 << 20, lazy_max_records: 256, lazy_max_ms: 1000, keep_old_snapshots: 1 }
    }
}

/// What `open` did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OpenReport {
    pub from_snapshot: Option<u64>,
    pub replayed: u64,
    pub recovery: Option<RecoveryInfo>,
    pub removed_unreferenced: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryInfo {
    pub segment: u32,
    pub last_good_seq: u64,
    pub truncated_bytes: u64,
    pub saved_as: Option<String>,
    pub detail: String,
}

// ------------------------------------------------------------------------------------------------------------
// The ledger
// ------------------------------------------------------------------------------------------------------------

pub struct Ledger {
    dir: PathBuf,
    cfg: LedgerConfig,
    clock: Arc<dyn Clock>,
    opener: Arc<dyn SegmentOpener>,
    manifest: Manifest,
    file: Box<dyn SegmentFile>,
    active_len: u64,
    active_hasher: blake3::Hasher,
    seq: u64,
    head: String,
    state: CampaignState,
    unsynced: usize,
    last_sync: Duration,
    poisoned: bool,
}

impl std::fmt::Debug for Ledger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ledger").field("dir", &self.dir).field("seq", &self.seq).field("head", &self.head).finish()
    }
}

/// Path of the ledger directory inside a campaign directory.
pub fn ledger_dir(cdir: &Path) -> PathBuf {
    cdir.join(LEDGER_DIR)
}

impl Ledger {
    /// Creates a new ledger in `cdir/ledger` and writes record 1 (`campaign_created`).
    pub fn create(
        cdir: &Path,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
        campaign_id: &str,
        spec_blake3: &str,
        terminal_bound: u32,
    ) -> Result<Ledger, LedgerError> {
        Self::create_with(cdir, clock, cfg, Arc::new(RealOpener), campaign_id, spec_blake3, terminal_bound)
    }

    pub fn create_with(
        cdir: &Path,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
        opener: Arc<dyn SegmentOpener>,
        campaign_id: &str,
        spec_blake3: &str,
        terminal_bound: u32,
    ) -> Result<Ledger, LedgerError> {
        let dir = ledger_dir(cdir);
        if dir.join(MANIFEST_FILE).exists() {
            return Err(LedgerError::AlreadyExists { dir });
        }
        fs::create_dir_all(&dir).map_err(io_err(&dir, "create dir"))?;
        let manifest = Manifest {
            schema: MANIFEST_SCHEMA.into(),
            campaign_id: campaign_id.into(),
            sealed: Vec::new(),
            active: ActiveSegment { index: 1, file: segment_name(1), first_seq: 1, prev: GENESIS.into() },
            snapshot: None,
        };
        let seg = dir.join(&manifest.active.file);
        // A crash between these two steps leaves an empty segment and no manifest: `create` again works after
        // the empty file is removed by the caller; `open` reports NotALedger.
        let file = opener.open(&seg).map_err(io_err(&seg, "create segment"))?;
        atomic::sync_dir(&dir).map_err(io_err(&dir, "sync dir"))?;
        write_manifest(&dir, &manifest)?;
        let now = clock.monotonic();
        let mut l = Ledger {
            dir,
            cfg,
            clock,
            opener,
            manifest,
            file,
            active_len: 0,
            active_hasher: blake3::Hasher::new(),
            seq: 0,
            head: GENESIS.into(),
            state: CampaignState::default(),
            unsynced: 0,
            last_sync: now,
            poisoned: false,
        };
        l.append(Event::CampaignCreated {
            campaign_id: campaign_id.into(),
            spec_blake3: spec_blake3.into(),
            terminal_bound,
        })?;
        Ok(l)
    }

    /// Opens an existing ledger: manifest, snapshot (if any), replay of the active segment, torn-tail recovery.
    ///
    /// Recovery algorithm (active segment):
    /// 1. read the segment; check every line in order: complete (ends with `\n`), parses, schema, `seq` dense,
    ///    `prev` = hash of the previous line, accepted by the reducer;
    /// 2. stop at the first line that fails; if ANY later `\n`-terminated line parses as a ledger record,
    ///    refuse (`Corrupt`: a person must look); otherwise the rest is a torn tail;
    /// 3. copy the torn bytes to `torn-<seq>-<utc ms>.bin`, truncate the segment to the last good record,
    ///    fsync, and append a `recovery` record (fsynced) saying how many bytes were dropped after which seq.
    ///
    /// A reducer refusal of a COMPLETE, chain-valid record is never treated as torn: `Replay` error.
    pub fn open(cdir: &Path, clock: Arc<dyn Clock>, cfg: LedgerConfig) -> Result<(Ledger, OpenReport), LedgerError> {
        Self::open_with(cdir, clock, cfg, Arc::new(RealOpener))
    }

    pub fn open_with(
        cdir: &Path,
        clock: Arc<dyn Clock>,
        cfg: LedgerConfig,
        opener: Arc<dyn SegmentOpener>,
    ) -> Result<(Ledger, OpenReport), LedgerError> {
        let dir = ledger_dir(cdir);
        let manifest = read_manifest(&dir)?;
        let mut report = OpenReport::default();
        report.removed_unreferenced = remove_unreferenced(&dir, &manifest)?;
        // State at the start of the active segment.
        let (mut state, mut seq, mut head) = match &manifest.snapshot {
            Some(s) => {
                let snap = load_snapshot(&dir, s)?;
                report.from_snapshot = Some(snap.seq);
                (snap.state, snap.seq, snap.head)
            }
            None => {
                let mut st = CampaignState::default();
                let mut seq = 0;
                let mut head = GENESIS.to_string();
                for s in &manifest.sealed {
                    let (n, h) = replay_sealed(&dir, s, &mut st, seq, &head, true)?;
                    report.replayed += n;
                    seq = s.last_seq;
                    head = h;
                }
                (st, seq, head)
            }
        };
        if seq + 1 != manifest.active.first_seq || head != manifest.active.prev {
            return Err(LedgerError::Corrupt {
                file: MANIFEST_FILE.into(),
                line: 0,
                why: format!("active segment starts at {} / {}, state ends at {seq} / {head}", manifest.active.first_seq, manifest.active.prev),
            });
        }
        // Active segment.
        let seg_path = dir.join(&manifest.active.file);
        let bytes = fs::read(&seg_path).map_err(io_err(&seg_path, "read segment"))?;
        let mut pos = 0usize;
        let mut line_no = 0u64;
        let mut hasher = blake3::Hasher::new();
        let mut torn: Option<String> = None;
        while pos < bytes.len() {
            line_no += 1;
            let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
                torn = Some(format!("incomplete last line ({} bytes, no newline)", bytes.len() - pos));
                break;
            };
            let line = &bytes[pos..pos + nl];
            match check_line(line, seq, &head) {
                Ok(rec) => {
                    state.apply_record(&rec).map_err(LedgerError::Replay)?;
                    seq = rec.seq;
                    head = line_hash(line);
                    hasher.update(&bytes[pos..pos + nl + 1]);
                    report.replayed += 1;
                    pos += nl + 1;
                }
                Err(why) => {
                    torn = Some(format!("line {line_no}: {why}"));
                    break;
                }
            }
        }
        if let Some(why) = &torn {
            // Refuse if any later complete line is a ledger record: that is not a torn tail.
            let rest = &bytes[pos..];
            let mut later_valid = false;
            // Skip the failing line itself, then look at every later newline-terminated line.
            if let Some(first_nl) = rest.iter().position(|&b| b == b'\n') {
                let mut p = first_nl + 1;
                while let Some(nl) = rest[p..].iter().position(|&b| b == b'\n') {
                    if serde_json::from_slice::<Record>(&rest[p..p + nl]).is_ok_and(|r| r.schema == LEDGER_SCHEMA) {
                        later_valid = true;
                        break;
                    }
                    p += nl + 1;
                }
            }
            if later_valid {
                return Err(LedgerError::Corrupt { file: manifest.active.file.clone(), line: line_no, why: format!("{why}; valid records follow") });
            }
        }
        let now_mono = clock.monotonic();
        let file = opener.open(&seg_path).map_err(io_err(&seg_path, "open segment"))?;
        let mut l = Ledger {
            dir: dir.clone(),
            cfg,
            clock,
            opener,
            manifest,
            file,
            active_len: pos as u64,
            active_hasher: hasher,
            seq,
            head,
            state,
            unsynced: 0,
            last_sync: now_mono,
            poisoned: false,
        };
        if let Some(why) = torn {
            let dropped = &bytes[pos..];
            let utc = l.clock.now_utc();
            let saved = format!("torn-{seq:012}-{}.bin", utc.0);
            atomic::create_new_atomic(&dir.join(&saved), dropped)?;
            l.file.truncate(pos as u64).map_err(io_err(&seg_path, "truncate torn tail"))?;
            let info = RecoveryInfo {
                segment: l.manifest.active.index,
                last_good_seq: seq,
                truncated_bytes: dropped.len() as u64,
                saved_as: Some(saved.clone()),
                detail: why.clone(),
            };
            l.append(Event::Recovery {
                segment: info.segment,
                last_good_seq: seq,
                truncated_bytes: info.truncated_bytes,
                saved_as: Some(saved),
                detail: why,
            })?;
            report.recovery = Some(info);
        }
        Ok((l, report))
    }

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

    /// Appends one record (validated by the reducer first: an illegal record is never written). Returns its seq.
    /// `Sync` records are durable when this returns (module doc).
    pub fn append(&mut self, event: Event) -> Result<u64, LedgerError> {
        let s = self.append_all(vec![event])?;
        Ok(s[0])
    }

    /// Appends several records with ONE fsync at the end if any of them is `Sync` (group commit).
    pub fn append_all(&mut self, events: Vec<Event>) -> Result<Vec<u64>, LedgerError> {
        if self.poisoned {
            return Err(LedgerError::Poisoned);
        }
        let start_len = self.active_len;
        let mut seqs = Vec::with_capacity(events.len());
        let mut need_sync = false;
        let mut buf = Vec::new();
        let mut lines: Vec<(usize, usize)> = Vec::new();
        let saved_state = if events.len() > 1 { Some(self.state.clone()) } else { None };
        let (mut seq, mut head) = (self.seq, self.head.clone());
        for ev in events {
            need_sync |= ev.durability() == Durability::Sync;
            let rec = Record { schema: LEDGER_SCHEMA.into(), seq: seq + 1, utc: self.clock.now_utc().to_iso(), prev: head.clone(), event: ev };
            let line = serde_json::to_vec(&rec).expect("records serialise");
            if line.len() > MAX_RECORD_BYTES {
                if let Some(s) = saved_state {
                    self.state = s;
                }
                return Err(LedgerError::TooLarge { bytes: line.len() });
            }
            if let Err(e) = self.state.apply_record(&rec) {
                if let Some(s) = saved_state {
                    self.state = s;
                }
                return Err(LedgerError::Refused(e));
            }
            seq = rec.seq;
            head = line_hash(&line);
            let off = buf.len();
            buf.extend_from_slice(&line);
            buf.push(b'\n');
            lines.push((off, buf.len()));
            seqs.push(seq);
        }
        let path = self.dir.join(&self.manifest.active.file);
        if let Err(e) = self.file.append(&buf) {
            // Roll the file back to the last complete record (shrinking needs no space), poison the handle.
            let _ = self.file.truncate(start_len);
            self.poisoned = true;
            return Err(io_err(&path, "append")(e));
        }
        self.active_len += buf.len() as u64;
        self.active_hasher.update(&buf);
        self.seq = seq;
        self.head = head;
        self.unsynced += lines.len();
        let now = self.clock.monotonic();
        let due = self.unsynced >= self.cfg.lazy_max_records
            || now.saturating_sub(self.last_sync) >= Duration::from_millis(self.cfg.lazy_max_ms);
        if need_sync || due {
            if let Err(e) = self.file.sync() {
                self.poisoned = true;
                return Err(io_err(&path, "fsync")(e));
            }
            self.unsynced = 0;
            self.last_sync = now;
        }
        if self.active_len >= self.cfg.rotate_bytes {
            self.rotate()?;
        }
        Ok(seqs)
    }

    /// fsync of the active segment (makes lazy records durable).
    pub fn sync(&mut self) -> Result<(), LedgerError> {
        if self.poisoned {
            return Err(LedgerError::Poisoned);
        }
        let path = self.dir.join(&self.manifest.active.file);
        self.file.sync().map_err(io_err(&path, "fsync"))?;
        self.unsynced = 0;
        self.last_sync = self.clock.monotonic();
        Ok(())
    }

    /// Seals the active segment, writes a snapshot of the reduced state, starts a new segment. Steps (the
    /// manifest write is the commit point; a crash before it leaves unreferenced files that `open` removes):
    /// 1. fsync active; 2. snapshot file (atomic); 3. new empty segment (create + dir fsync); 4. manifest
    /// (atomic); 5. old segment chmod 0444; 6. `segment_opened` record in the new segment; 7. prune snapshots.
    pub fn rotate(&mut self) -> Result<(), LedgerError> {
        if self.poisoned {
            return Err(LedgerError::Poisoned);
        }
        if self.active_len == 0 {
            return Ok(());
        }
        self.sync()?;
        let old = self.manifest.active.clone();
        let seg_blake3 = self.active_hasher.clone().finalize().to_hex().to_string();
        let snap = Snapshot { schema: SNAPSHOT_SCHEMA.into(), seq: self.seq, head: self.head.clone(), state: self.state.clone() };
        let snap_name = snapshot_name(self.seq);
        let mut snap_bytes = serde_json::to_vec(&snap).expect("snapshot serialises");
        snap_bytes.push(b'\n');
        atomic::write_atomic(&self.dir.join(&snap_name), &snap_bytes)?;
        let new_index = old.index + 1;
        let new_name = segment_name(new_index);
        let new_path = self.dir.join(&new_name);
        let new_file = self.opener.open(&new_path).map_err(io_err(&new_path, "create segment"))?;
        atomic::sync_dir(&self.dir).map_err(io_err(&self.dir, "sync dir"))?;
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
        m.active = ActiveSegment { index: new_index, file: new_name, first_seq: self.seq + 1, prev: self.head.clone() };
        let old_snapshot = m.snapshot.replace(SnapshotRef {
            file: snap_name,
            seq: self.seq,
            blake3: blake3::hash(&snap_bytes).to_hex().to_string(),
            state_digest: self.state.digest(),
        });
        write_manifest(&self.dir, &m)?;
        self.manifest = m;
        let _ = fs::set_permissions(self.dir.join(&old.file), fs::Permissions::from_mode(0o444));
        self.file = new_file;
        self.active_len = 0;
        self.active_hasher = blake3::Hasher::new();
        let snapshot_seq = self.seq;
        self.append(Event::SegmentOpened { index: new_index, previous_blake3: seg_blake3, snapshot_seq })?;
        // Prune snapshots older than the kept ones (never the manifest's).
        let _ = old_snapshot;
        prune_snapshots(&self.dir, &self.manifest, self.cfg.keep_old_snapshots);
        Ok(())
    }
}

fn check_line(line: &[u8], seq: u64, head: &str) -> Result<Record, String> {
    let rec: Record = serde_json::from_slice(line).map_err(|e| format!("unparsable: {e}"))?;
    if rec.schema != LEDGER_SCHEMA {
        return Err(format!("unknown schema {:?}", rec.schema));
    }
    if rec.seq != seq + 1 {
        return Err(format!("seq {} after {seq}", rec.seq));
    }
    if rec.prev != head {
        return Err("hash chain broken (prev does not match the previous record)".into());
    }
    Ok(rec)
}

fn write_manifest(dir: &Path, m: &Manifest) -> Result<(), LedgerError> {
    atomic::write_json_atomic(&dir.join(MANIFEST_FILE), m).map_err(LedgerError::from)
}

pub fn read_manifest(dir: &Path) -> Result<Manifest, LedgerError> {
    let p = dir.join(MANIFEST_FILE);
    if !p.exists() {
        return Err(LedgerError::NotALedger { dir: dir.to_path_buf() });
    }
    let bytes = atomic::read_capped(&p, 64 << 20)?;
    let m: Manifest = serde_json::from_slice(&bytes)
        .map_err(|e| LedgerError::Corrupt { file: MANIFEST_FILE.into(), line: 0, why: e.to_string() })?;
    if m.schema != MANIFEST_SCHEMA {
        return Err(LedgerError::Corrupt { file: MANIFEST_FILE.into(), line: 0, why: format!("schema {:?}", m.schema) });
    }
    Ok(m)
}

fn load_snapshot(dir: &Path, s: &SnapshotRef) -> Result<Snapshot, LedgerError> {
    let p = dir.join(&s.file);
    let bytes = atomic::read_capped(&p, 16 << 30)?;
    if blake3::hash(&bytes).to_hex().as_str() != s.blake3 {
        return Err(LedgerError::Corrupt { file: s.file.clone(), line: 0, why: "snapshot digest differs from the manifest".into() });
    }
    let snap: Snapshot =
        serde_json::from_slice(&bytes).map_err(|e| LedgerError::Corrupt { file: s.file.clone(), line: 0, why: e.to_string() })?;
    if snap.schema != SNAPSHOT_SCHEMA || snap.seq != s.seq {
        return Err(LedgerError::Corrupt { file: s.file.clone(), line: 0, why: "snapshot schema or seq mismatch".into() });
    }
    Ok(snap)
}

/// Replays one sealed segment into `st`, checking its digest (if `check_digest`), sequence and chain.
/// Returns (records, hash of the last record).
fn replay_sealed(
    dir: &Path,
    s: &SealedSegment,
    st: &mut CampaignState,
    mut seq: u64,
    head: &str,
    check_digest: bool,
) -> Result<(u64, String), LedgerError> {
    let p = dir.join(&s.file);
    let bytes = fs::read(&p).map_err(io_err(&p, "read sealed segment"))?;
    if check_digest && blake3::hash(&bytes).to_hex().as_str() != s.blake3 {
        return Err(LedgerError::Corrupt { file: s.file.clone(), line: 0, why: "sealed segment digest differs from the manifest (edited or truncated)".into() });
    }
    let mut head = head.to_string();
    let mut n = 0u64;
    let mut pos = 0usize;
    while pos < bytes.len() {
        let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') else {
            return Err(LedgerError::Corrupt { file: s.file.clone(), line: n + 1, why: "sealed segment ends inside a line".into() });
        };
        let line = &bytes[pos..pos + nl];
        let rec = check_line(line, seq, &head).map_err(|why| LedgerError::Corrupt { file: s.file.clone(), line: n + 1, why })?;
        st.apply_record(&rec).map_err(LedgerError::Replay)?;
        seq = rec.seq;
        head = line_hash(line);
        n += 1;
        pos += nl + 1;
    }
    if seq != s.last_seq || head != s.last_hash {
        return Err(LedgerError::Corrupt { file: s.file.clone(), line: n, why: "sealed segment ends elsewhere than the manifest says".into() });
    }
    Ok((n, head))
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
        if let Some(idx) = name.strip_prefix("segment-").and_then(|r| r.strip_suffix(".jsonl")).and_then(|n| n.parse::<u32>().ok()) {
            if idx > m.active.index {
                let len = e.metadata().map(|md| md.len()).unwrap_or(1);
                if len != 0 {
                    return Err(LedgerError::Corrupt { file: name, line: 0, why: "non-empty segment not in the manifest".into() });
                }
                fs::remove_file(e.path()).map_err(io_err(dir, "remove"))?;
                removed.push(name);
            }
        } else if let Some(sq) = name.strip_prefix("snapshot-").and_then(|r| r.strip_suffix(".json")).and_then(|n| n.parse::<u64>().ok()) {
            if m.snapshot.as_ref().is_none_or(|s| sq > s.seq) {
                fs::remove_file(e.path()).map_err(io_err(dir, "remove"))?;
                removed.push(name);
            }
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
        .filter_map(|e| e.file_name().to_str()?.strip_prefix("snapshot-")?.strip_suffix(".json")?.parse::<u64>().ok())
        .filter(|&s| s < cur.seq)
        .collect();
    old.sort_unstable();
    let n = old.len().saturating_sub(keep_old);
    for s in &old[..n] {
        let _ = fs::remove_file(dir.join(snapshot_name(*s)));
    }
}

// ------------------------------------------------------------------------------------------------------------
// Full reading (verify, tests, tools)
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
}

/// Reads EVERY record from segment 1 (ignoring snapshots), verifying sealed digests, sequence and the hash
/// chain across segments, reducing as it goes; compares the snapshot. Read-only: never repairs.
pub fn verify_chain(cdir: &Path) -> Result<ChainReport, LedgerError> {
    let dir = ledger_dir(cdir);
    let m = read_manifest(&dir)?;
    let mut st = CampaignState::default();
    let (mut seq, mut head) = (0u64, GENESIS.to_string());
    let mut n = 0;
    let mut snapshot_matches = None;
    for s in &m.sealed {
        let (k, h) = replay_sealed(&dir, s, &mut st, seq, &head, true)?;
        n += k;
        seq = s.last_seq;
        head = h;
        if let Some(sr) = &m.snapshot {
            if sr.seq == seq {
                let snap = load_snapshot(&dir, sr)?;
                snapshot_matches = Some(snap.state == st && snap.head == head && sr.state_digest == st.digest());
            }
        }
    }
    let p = dir.join(&m.active.file);
    let mut bytes = Vec::new();
    File::open(&p).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(io_err(&p, "read"))?;
    let mut pos = 0;
    let mut line_no = 0u64;
    while let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') {
        line_no += 1;
        let line = &bytes[pos..pos + nl];
        let rec = check_line(line, seq, &head).map_err(|why| LedgerError::Corrupt { file: m.active.file.clone(), line: line_no, why })?;
        st.apply_record(&rec).map_err(LedgerError::Replay)?;
        seq = rec.seq;
        head = line_hash(line);
        n += 1;
        pos += nl + 1;
    }
    Ok(ChainReport {
        records: n,
        segments: m.sealed.len() as u32 + 1,
        head,
        state: st,
        snapshot_matches,
        torn_tail_bytes: (bytes.len() - pos) as u64,
    })
}

/// All complete records of the ledger in order (no reduction; for tools and tests).
pub fn read_records(cdir: &Path) -> Result<Vec<Record>, LedgerError> {
    let dir = ledger_dir(cdir);
    let m = read_manifest(&dir)?;
    let mut out = Vec::new();
    let files: Vec<String> = m.sealed.iter().map(|s| s.file.clone()).chain(std::iter::once(m.active.file.clone())).collect();
    for f in files {
        let p = dir.join(&f);
        let bytes = fs::read(&p).map_err(io_err(&p, "read"))?;
        let mut pos = 0;
        while let Some(nl) = bytes[pos..].iter().position(|&b| b == b'\n') {
            let rec: Record = serde_json::from_slice(&bytes[pos..pos + nl])
                .map_err(|e| LedgerError::Corrupt { file: f.clone(), line: 0, why: e.to_string() })?;
            out.push(rec);
            pos += nl + 1;
        }
    }
    Ok(out)
}

/// Content id helper: first 32 hex characters of BLAKE3 over `parts` joined by NUL.
pub fn content_id(parts: &[&str]) -> String {
    let mut h = blake3::Hasher::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            h.update(&[0]);
        }
        h.update(p.as_bytes());
    }
    h.finalize().to_hex()[..32].to_string()
}
