//! Durable coordinator state, not a completed reduction artifact.
//!
//! CP5 layout (flat, generation-suffixed): `latest.json`/`previous.json` are
//! the manifests; `meta-<G>.json`, `nodes-<G>.bin`, `ledger-<G>.bin` and
//! `index-<G>.bin` are rewritten each generation; `domains-<S>.bin`,
//! `edges-<S>.bin` and `records-<S>.jsonl` are append-only segments that tile
//! their section, so a save costs O(new state) plus the small sections. A
//! checkpointed walk streams its records into the open `records-<G>.jsonl`
//! at commit time (`execution/records.rs`); the save seals that segment
//! instead of writing records from RAM. Every digest is computed while
//! writing; resume verifies every referenced file in parallel before
//! decoding. Resume is bound to the request/policy digest,
//! the owner digests and `WALK_SEMANTICS_VERSION`; the executable digest is
//! recorded and reported, never a refusal.
#[cfg(test)]
mod finite_replay_tests;
pub(super) mod manifest;
#[cfg(test)]
mod preferred_subset_tests;
#[cfg(all(test, feature = "cli"))]
mod reinspection_tests;
pub(super) mod restore;
#[cfg(all(test, feature = "cli"))]
mod root_blockers_tests;
#[cfg(all(test, feature = "cli"))]
mod scale_tests;
pub(super) mod sections;
#[cfg(test)]
pub(super) mod test_support;

use super::{
    OwnerDomainWalkFrontierPolicy, OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest,
    OwnerDomainWalkSchedulingPolicy, WALK_SEMANTICS_VERSION,
    execution::{
        ChangeStamp, State,
        records::{RecordSink, Sidecar},
    },
};
use crate::application::atomic_file::{write_file_atomically, write_file_atomically_with};
/// Bound every reader of `latest.json` shares with the store.
pub use manifest::MAX_MANIFEST_BYTES as OWNER_DOMAIN_WALK_CHECKPOINT_MANIFEST_MAX_BYTES;
use manifest::{FORMAT, Manifest, SCHEMA, Section, SectionRef, Sections, Segment, Segmented};
/// Manifest `format`/`schema` this executable writes and resumes.
pub use manifest::{
    FORMAT as OWNER_DOMAIN_WALK_CHECKPOINT_FORMAT, SCHEMA as OWNER_DOMAIN_WALK_CHECKPOINT_SCHEMA,
};
pub(super) use restore::Restored;
use sections::{HashingWriter, Identity};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

/// The role of one save. `Periodic` waits for the interval; `Forced` and
/// `Final` write unless nothing changed. Only a save after which the walk
/// continues folds the persisted edge log into the CSR: after the final save
/// the process reports and exits, and a resume rebuilds a folded CSR anyway,
/// so a fold there would only add an uncancellable allocation spike.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SaveKind {
    Periodic,
    Forced,
    Final,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnerDomainWalkCheckpointOptions {
    pub directory: PathBuf,
    pub resume: bool,
    pub interval_seconds: u64,
}
impl OwnerDomainWalkCheckpointOptions {
    pub fn new(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
            resume: false,
            interval_seconds: 3600,
        }
    }
}
pub(super) struct Store {
    options: OwnerDomainWalkCheckpointOptions,
    _lock: File,
    manifest: Option<Manifest>,
    request: String,
    policy: String,
    /// Responsibility-transfer lookahead the request demands (None for
    /// InspectAll); the restored ledger must agree, not only the digest.
    lookahead: Option<std::num::NonZeroUsize>,
    executable: String,
    semantics: u32,
    owners: Vec<String>,
    last: Instant,
    last_save_seconds: f64,
    last_stamp: Option<ChangeStamp>,
    verify_seconds: f64,
    pending_events: Vec<Value>,
    /// G2' activation record (a binding amendment from off to union): set by
    /// the activating resume and inherited from the manifest metadata by every
    /// later session, so each later generation repeats it.
    g2_activation: Option<Value>,
    /// This session activated G2' on a checkpoint written without it.
    g2_activating: bool,
    /// Label of a diagnostic pause this process triggered; every later save
    /// of the session repeats it in the manifest metadata (a free-form
    /// object, so older readers and older manifests are unaffected).
    diagnostic_pause: Option<&'static str>,
    /// Stop reason of a policy stop this process triggered (the A10 frontier
    /// stop); like `diagnostic_pause`, optional free-form metadata repeated by
    /// every later save of the session and never inherited by a resume.
    stop_reason: Option<&'static str>,
    /// The rescue amendment chain (`rescue.rs`): the resumed manifest's,
    /// extended by the amendments this session applied; every later
    /// manifest carries it.
    amendments: Vec<super::rescue::AmendmentRef>,
    #[cfg(test)]
    fail_section: Option<Section>,
    #[cfg(test)]
    fail_cleanup: bool,
}
fn binding(request: &OwnerDomainWalkRequest) -> String {
    blake3::hash(binding_value(request).to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// Authenticate the one supported request transition using the actual bound
/// request, not a digest asserted by saved metadata. No other request fields
/// (including query roles and bounds) may change at G2' activation.
fn g2_activation_before(
    request: &OwnerDomainWalkRequest,
    current: &str,
    activation: Option<&Value>,
) -> Result<Option<String>, String> {
    let Some(activation) = activation.filter(|v| !v.is_null()) else {
        return Ok(None);
    };
    let mut off = request.clone();
    off.g2_residual_anchors = super::OwnerDomainWalkG2ResidualAnchors::Off;
    let before = binding(&off);
    if request.g2_residual_anchors != super::OwnerDomainWalkG2ResidualAnchors::Union
        || current != binding(request)
        || activation["from"] != "off"
        || activation["to"] != "union"
        || activation["binding_before"].as_str() != Some(before.as_str())
        || activation["binding_after"].as_str() != Some(current)
    {
        return Err("invalid G2' activation request binding receipt".into());
    }
    Ok(Some(before))
}

/// One immutable rescue chain base, shared by walker, verifier and planner.
/// Only an already-recorded chain may retain the authenticated pre-activation
/// binding. A first amendment created after activation binds the current one.
pub(super) fn rescue_chain_base(
    request: &OwnerDomainWalkRequest,
    current: &str,
    activation: Option<&Value>,
    recorded: &[super::rescue::AmendmentRef],
) -> Result<String, String> {
    let before = g2_activation_before(request, current, activation)?;
    Ok(match before {
        Some(before) if recorded.first().is_some_and(|first| first.parent == before) => before,
        _ => current.to_owned(),
    })
}
/// The bound request value (see `binding`).
fn binding_value(request: &OwnerDomainWalkRequest) -> Value {
    // Checkpoint location, interval and resume mode are transport, not policy.
    let mut value = json!({"selection":request.matching.selection_json,"queries":request.matching.queries_json,
        "limits":super::limits_json(request),"reduction":format!("{:?}",request.matching.reduction_limits),
        "workers":request.workers,"inspection_workers":request.inspection_workers,
        "publication":format!("{:?}",request.publication_policy),"scheduling":format!("{:?}",request.scheduling_policy),
        "reuse_initial_d_bands":request.reuse_initial_d_bands,"max_domains":request.max_domains,
        "max_events":request.max_events,"max_frontiers":request.max_frontiers,
        "max_containment_checks":request.max_containment_checks,"route_domain_overcover":request.route_domain_overcover,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "max_route_masks":request.max_route_masks,"subdivision":request.apply_subdivision,
        "max_queries":request.matching.max_queries,"max_query_bytes":request.matching.max_query_bytes});
    add_preferred_subset_binding(&mut value, request);
    add_finite_replay_binding(&mut value, request);
    // A10: the stop policy is semantic and bound; Record (the historical
    // behaviour) adds no key, so every existing CP5 binding is unchanged.
    if request.frontier_policy != OwnerDomainWalkFrontierPolicy::Record {
        value["frontier_policy"] = json!(request.frontier_policy.name());
    }
    // G2' residual anchors change which records exist: bound; Off adds no key.
    if request.g2_residual_anchors != super::OwnerDomainWalkG2ResidualAnchors::Off {
        value["g2_residual_anchors"] = json!(request.g2_residual_anchors.name());
    }
    value
}

fn add_preferred_subset_binding(value: &mut Value, request: &OwnerDomainWalkRequest) {
    // Older binaries ignored unknown selection fields while hashing the same
    // raw JSON. A NEW marker outside that raw string prevents those old
    // all-preferred checkpoints from masquerading as subset-policy states.
    if super::super::input::has_preferred_rule_subsets(&request.matching.selection_json) {
        value["preferred_rule_subset_policy"] =
            super::super::input::PREFERRED_RULE_SUBSET_POLICY.into();
    }
}
fn add_finite_replay_binding(value: &mut Value, request: &OwnerDomainWalkRequest) {
    // No key in the flag-off path: historical request identity stays intact.
    // The explicit version prevents old readers treating an unknown recipe
    // as an ordinary locally empty inspection under an identical binding.
    if let Some(limits) = request.finite_replay {
        value["finite_replay"] = json!({
            "version": super::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION,
            "scope": "whole_initial_id0_finite_domain",
            "limits": limits,
        });
    }
}
fn policy_name(policy: OwnerDomainWalkPublicationPolicy) -> &'static str {
    match policy {
        OwnerDomainWalkPublicationPolicy::Ordered => "ordered",
        OwnerDomainWalkPublicationPolicy::Ready => "ready",
        OwnerDomainWalkPublicationPolicy::OwnerBatched => "owner_batched",
        OwnerDomainWalkPublicationPolicy::Epoch => "epoch",
    }
}
fn unix_time() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs())
}
#[cfg(target_os = "linux")]
fn resident_set_bytes() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    let line = status.lines().find(|line| line.starts_with("VmRSS:"))?;
    let kibibytes = line.split_ascii_whitespace().nth(1)?.parse::<u64>().ok()?;
    kibibytes.checked_mul(1_024)
}
#[cfg(not(target_os = "linux"))]
fn resident_set_bytes() -> Option<u64> {
    None
}
/// Stream one section to its final name; the digest is taken from the bytes
/// as they are written, never from a second read of the file.
fn write_section(
    path: &Path,
    write: impl FnOnce(&mut dyn Write) -> Result<(), String>,
) -> Result<(u64, String), String> {
    let mut digest = None;
    write_file_atomically_with(path, false, |file| {
        let mut out = HashingWriter::new(BufWriter::with_capacity(1 << 20, file));
        write(&mut out)?;
        digest = Some(
            out.finish()
                .map_err(|e| format!("cannot finish {}: {e}", path.display()))?,
        );
        Ok(())
    })?;
    digest.ok_or_else(|| "checkpoint section digest missing".into())
}
/// Smallest generation above `after` with no section file of ours: orphans
/// of a failed save or a crashed sidecar keep their generation to themselves.
pub(super) fn next_free_generation(directory: &Path, after: u64) -> Result<u64, String> {
    let mut generation = after
        .checked_add(1)
        .ok_or("checkpoint generation overflow")?;
    while Section::ALL
        .iter()
        .any(|s| directory.join(s.file_name(generation)).exists())
    {
        generation = generation
            .checked_add(1)
            .ok_or("checkpoint generation overflow")?;
    }
    Ok(generation)
}
/// Segments retained from the previous generation plus the new tail.
struct Plan {
    keep: Vec<Segment>,
    first: usize,
    count: usize,
}
fn plan(previous: Option<&Segmented>, total: usize) -> Plan {
    match previous {
        Some(p) if usize::try_from(p.total).is_ok_and(|t| t <= total) => Plan {
            keep: p.segments.clone(),
            first: p.total as usize,
            count: total - p.total as usize,
        },
        // Nothing retained (fresh store, or the section shrank because the
        // dependency monitor released its graph): re-tile from zero.
        _ => Plan {
            keep: Vec::new(),
            first: 0,
            count: total,
        },
    }
}
struct Written {
    section: Section,
    file: String,
    first: usize,
    count: usize,
    bytes: u64,
    blake3: String,
    seconds: f64,
}
type Writer<'a> = Box<dyn FnOnce(&mut dyn Write) -> Result<(), String> + Send + 'a>;
impl Store {
    pub(super) fn open(request: &OwnerDomainWalkRequest) -> Result<Option<Self>, String> {
        if request.checkpoint.is_none() {
            return Ok(None);
        }
        let executable =
            restore::file_digest(&std::env::current_exe().map_err(|e| e.to_string())?)?.1;
        Self::open_with_identity(request, executable, WALK_SEMANTICS_VERSION)
    }
    /// Identity seam: production passes the running executable's digest and
    /// `WALK_SEMANTICS_VERSION`; tests substitute both.
    pub(super) fn open_with_identity(
        request: &OwnerDomainWalkRequest,
        executable: String,
        semantics: u32,
    ) -> Result<Option<Self>, String> {
        let Some(options) = request.checkpoint.clone() else {
            return Ok(None);
        };
        if options.resume {
            if !options.directory.is_dir() {
                return Err("resume checkpoint directory does not exist".into());
            }
        } else if options.directory.exists() {
            if fs::read_dir(&options.directory)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
            {
                return Err("new checkpoint directory must be empty".into());
            }
        } else {
            fs::create_dir(&options.directory)
                .map_err(|e| format!("cannot create checkpoint directory: {e}"))?;
        }
        if fs::symlink_metadata(&options.directory)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("checkpoint directory may not be a symlink".into());
        }
        if !options.resume {
            // The generation writer syncs this directory's contents. Its own
            // entry in the parent must also survive a crash after first save.
            let parent = options
                .directory
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .unwrap_or_else(|| Path::new("."));
            File::open(parent)
                .and_then(|directory| directory.sync_all())
                .map_err(|error| format!("cannot sync checkpoint directory parent: {error}"))?;
        }
        let lock_path = options.directory.join("checkpoint.lock");
        if fs::symlink_metadata(&lock_path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("checkpoint lock may not be a symlink".into());
        }
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("checkpoint is in use: {e}"))?;
        let request_binding = binding(request);
        let policy = policy_name(request.publication_policy).to_owned();
        let lookahead = match request.scheduling_policy {
            OwnerDomainWalkSchedulingPolicy::TransferUnreserved { lookahead } => Some(lookahead),
            OwnerDomainWalkSchedulingPolicy::InspectAll => None,
        };
        let mut pending_events = Vec::new();
        let mut verify_seconds = 0.0;
        let mut g2_activation = None;
        let mut g2_activating = false;
        let manifest = if options.resume {
            let m = manifest::read(&options.directory.join("latest.json"))?;
            g2_activation = Some(m.metadata["g2_activation"].clone()).filter(|v| !v.is_null());
            // G2' activation: a checkpoint bound to this request with G2' off
            // (and never written with G2') may continue with G2' on. The
            // amendment is recorded in every later generation's metadata.
            let mut request_binding = request_binding.clone();
            if request.g2_activate_on_resume
                && m.request != request_binding
                && m.kind == "state"
                && m.sections.anchors.is_none()
            {
                let mut off = request.clone();
                off.g2_residual_anchors = super::OwnerDomainWalkG2ResidualAnchors::Off;
                if m.request == binding(&off) {
                    g2_activation = Some(
                        json!({"from":"off","to":request.g2_residual_anchors.name(),
                        "generation":m.generation,"binding_before":m.request,"binding_after":request_binding,
                        "committed_domains":m.metadata["committed_domains"],"activated_unix_time":unix_time()?}),
                    );
                    g2_activating = true;
                    pending_events.push(
                        json!({"event":"g2_activation_requested","operation":"owner_domain_walk",
                        "generation":m.generation,"family_closure_claim":false}),
                    );
                    request_binding = m.request.clone();
                }
            }
            g2_activation_before(request, &binding(request), g2_activation.as_ref())?;
            // The ledger section is optional in the manifest, so its presence
            // is bound to the request here; a manifest without it must not
            // resume a transfer campaign as InspectAll.
            if m.request != request_binding
                || m.publication_policy != policy
                || (m.kind == "state" && m.sections.ledger.is_some() != lookahead.is_some())
            {
                return Err("checkpoint request or policy differs; refusing to restart".into());
            }
            if m.walk_semantics_version != semantics {
                return Err(format!(
                    "checkpoint walk semantics version differs (saved {}, executable {semantics}); resume requires identical admission/matching/routing semantics",
                    m.walk_semantics_version
                ));
            }
            if m.executable != executable {
                pending_events.push(json!({"event":"checkpoint_executable_changed","operation":"owner_domain_walk",
                    "saved":m.executable,"current":executable,"executable_first":m.executable_first,
                    "generation":m.generation,"walk_semantics_version":semantics,"family_closure_claim":false}));
            }
            verify_seconds = restore::verify_files(&options.directory, &m)?;
            Some(m)
        } else {
            None
        };
        let amendments = manifest
            .as_ref()
            .map(|m| m.amendments.clone())
            .unwrap_or_default();
        Ok(Some(Self {
            options,
            _lock: lock,
            manifest,
            request: request_binding,
            policy,
            lookahead,
            executable,
            semantics,
            owners: Vec::new(),
            last: Instant::now(),
            last_save_seconds: 0.0,
            last_stamp: None,
            verify_seconds,
            pending_events,
            g2_activation,
            g2_activating,
            diagnostic_pause: None,
            stop_reason: None,
            amendments,
            #[cfg(test)]
            fail_section: None,
            #[cfg(test)]
            fail_cleanup: false,
        }))
    }
    /// Events produced while opening (before the caller's observer existed).
    pub(super) fn take_open_events(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.pending_events)
    }
    pub(super) fn bind_owners(&mut self, owners: Vec<String>) -> Result<(), String> {
        if self
            .manifest
            .as_ref()
            .is_some_and(|m| m.kind == "state" && m.owners != owners)
        {
            return Err("checkpoint immutable owner payload bytes differ".into());
        }
        self.owners = owners;
        Ok(())
    }
    pub(super) fn resume<const N: usize>(
        &mut self,
        observer: &impl Fn(Value),
    ) -> Result<Option<Restored<N>>, String> {
        let Some(m) = &self.manifest else {
            return Ok(None);
        };
        if !self.options.resume || m.kind == "bootstrap" {
            return Ok(None);
        }
        let started = Instant::now();
        let restored = restore::restore::<N>(&self.options.directory, m, self.verify_seconds)?;
        if restored
            .state
            .queue
            .delegation
            .as_ref()
            .map(|ledger| ledger.lookahead())
            != self.lookahead
        {
            return Err(
                "checkpoint request or policy differs; restored responsibility ledger does not match the scheduling policy".into(),
            );
        }
        // The forced save after resume is free unless the walk changes state.
        // The restored state is not paused; the retained manifest may say it
        // was, so continuing the walk flips the flag with exactly one write.
        let mut stamp = restored.state.change_stamp();
        stamp.paused = m.metadata["paused"] == true;
        // Activation changes the request binding, G2 log and pinned decisions
        // without necessarily publishing any new domain. The first forced
        // save must durably record that transition even for a drained walk.
        // A successful save installs its normal stamp; ordinary resumes keep
        // the historical unchanged-prefix fast path.
        self.last_stamp = (!self.g2_activating).then_some(stamp);
        let mut report = restored.report.clone();
        report["directory"] = json!(self.options.directory);
        report["generation"] = json!(m.generation);
        report["bytes"] = json!(m.total_bytes());
        report["restore_seconds"] = json!(started.elapsed().as_secs_f64());
        report["rss_bytes"] = json!(resident_set_bytes());
        report["executable_changed_since_bootstrap"] = json!(m.executable_first != self.executable);
        observer(
            json!({"event":"checkpoint_restored","operation":"owner_domain_walk",
            "restore":report,"family_closure_claim":false}),
        );
        Ok(Some(restored))
    }
    /// Whether this session activates G2' on a checkpoint written without it.
    pub(super) fn g2_activating(&self) -> bool {
        self.g2_activating
    }
    pub(super) fn g2_activation(&self) -> Option<&Value> {
        self.g2_activation.as_ref()
    }
    pub(super) fn metadata(&self) -> Option<&Value> {
        self.manifest.as_ref().map(|m| &m.metadata)
    }
    /// Label every later save of this session as a diagnostic pause.
    pub(super) fn mark_diagnostic_pause(&mut self, label: &'static str) {
        self.diagnostic_pause = Some(label);
    }
    /// Label every later save of this session with a policy stop reason.
    /// The next save writes a labelled generation even when the walk state
    /// is unchanged since the last one (e.g. a stop on initial input
    /// frontiers right after the forced first save).
    pub(super) fn mark_stop_reason(&mut self, reason: &'static str) {
        self.stop_reason = Some(reason);
        self.last_stamp = None;
    }
    /// The rescue amendment chain recorded so far (see the field).
    pub(super) fn amendments(&self) -> &[super::rescue::AmendmentRef] {
        &self.amendments
    }
    /// Current request digest; a pre-activation rescue chain can retain its
    /// separately authenticated original base (`rescue_chain_base`).
    pub(super) fn request_digest(&self) -> &str {
        &self.request
    }
    /// The generation of the retained manifest (0 before the first save).
    pub(super) fn generation(&self) -> u64 {
        self.manifest.as_ref().map_or(0, |m| m.generation)
    }
    /// Whether the resumed generation holds walk state (not a bootstrap).
    pub(super) fn resumed_state(&self) -> bool {
        self.options.resume && self.manifest.as_ref().is_some_and(|m| m.kind == "state")
    }
    /// Append one applied amendment; the next save (forced right after the
    /// resume) persists it atomically with the admitted domains and inputs.
    pub(super) fn record_amendment(&mut self, amendment: super::rescue::AmendmentRef) {
        self.amendments.push(amendment);
        self.last_stamp = None;
    }
    fn effective_interval(&self) -> f64 {
        (self.options.interval_seconds as f64).max(20.0 * self.last_save_seconds)
    }
    fn identity_metadata(&self, executable_first: &str) -> Value {
        json!({"executable":self.executable,
            "executable_changed_since_bootstrap":executable_first != self.executable,
            "walk_semantics_version":self.semantics,"format":FORMAT,
            "effective_interval_seconds":self.effective_interval()})
    }
    pub(super) fn bootstrap(&mut self) -> Result<Option<Value>, String> {
        if self.manifest.is_some() {
            return Ok(None);
        }
        let generation = 1;
        let file = Section::Meta.file_name(generation);
        let path = self.options.directory.join(&file);
        let (bytes, blake3) = write_section(&path, |out| {
            out.write_all(b"{\"bootstrap\":true}\n")
                .map_err(|e| e.to_string())
        })?;
        let mut metadata = json!({"state":"saved","directory":self.options.directory,"generation":generation,"state_path":path,
            "saved_unix_time":unix_time()?,
            "committed_domains":0,"pending_domains":0,"completed_native_inspections":0,"committed_events":0,
            "paused":false,"bootstrap":true,"preparation_must_restart":true,"bytes":bytes,"new_bytes":bytes});
        merge(&mut metadata, self.identity_metadata(&self.executable));
        let manifest = Manifest {
            schema: SCHEMA,
            format: FORMAT.into(),
            kind: "bootstrap".into(),
            generation,
            walk_semantics_version: self.semantics,
            arity: 0,
            publication_policy: self.policy.clone(),
            request: self.request.clone(),
            owners: Vec::new(),
            executable: self.executable.clone(),
            executable_first: self.executable.clone(),
            sections: Sections {
                meta: Some(SectionRef {
                    file,
                    bytes,
                    blake3,
                }),
                ..Default::default()
            },
            metadata: metadata.clone(),
            amendments: Vec::new(),
        };
        self.publish(manifest)?;
        Ok(Some(
            json!({"event":"checkpoint_saved","operation":"owner_domain_walk","checkpoint":metadata,"family_closure_claim":false}),
        ))
    }
    /// Install the manifest (fatal on failure), then reclaim superseded files
    /// best-effort; the returned strings describe cleanup failures only.
    fn publish(&mut self, manifest: Manifest) -> Result<Vec<String>, String> {
        // Retain the complete preceding authority, not merely its state bytes.
        // Recovery is explicit: a corrupt latest generation is never silently
        // replaced by an older prefix during normal --resume admission.
        if let Some(previous) = &self.manifest {
            write_file_atomically(
                &self.options.directory.join("previous.json"),
                &serde_json::to_vec(previous).map_err(|e| e.to_string())?,
                true,
            )?;
        }
        write_file_atomically(
            &self.options.directory.join("latest.json"),
            &serde_json::to_vec(&manifest).map_err(|e| e.to_string())?,
            true,
        )?;
        let previous = self.manifest.replace(manifest);
        self.last = Instant::now();
        // The new generation is durable from here on. A failed save returns
        // before this point, keeping both the old authority and any newly
        // written orphan; a failed cleanup must not turn a saved generation
        // into a failed one, so it is reported and retried at the next save.
        Ok(previous.map_or_else(Vec::new, |previous| self.cleanup(&previous)))
    }
    /// Only our own validated section names below the previous good
    /// generation, and only when neither retained manifest references them,
    /// are ours to remove.
    fn cleanup(&self, previous: &Manifest) -> Vec<String> {
        let mut errors = Vec::new();
        let latest = self.manifest.as_ref().expect("published manifest");
        let referenced: HashSet<&str> = latest
            .files()
            .into_iter()
            .chain(previous.files())
            .map(|f| f.file)
            .collect();
        let entries = match fs::read_dir(&self.options.directory) {
            Ok(entries) => entries,
            Err(e) => return vec![format!("cannot list checkpoint directory: {e}")],
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    errors.push(format!("cannot read checkpoint directory entry: {e}"));
                    continue;
                }
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some((_, generation)) = Section::parse(&name) else {
                continue;
            };
            if generation >= previous.generation || referenced.contains(name.as_ref()) {
                continue;
            }
            let is_file = entry.file_type().map(|kind| kind.is_file());
            #[cfg(test)]
            let is_file = if self.fail_cleanup {
                Err(std::io::Error::other("injected cleanup failure"))
            } else {
                is_file
            };
            match is_file {
                Ok(true) => {
                    if let Err(e) = fs::remove_file(entry.path()) {
                        errors.push(format!("cannot remove superseded {name}: {e}"));
                    }
                }
                Ok(false) => {}
                Err(e) => errors.push(format!("cannot inspect superseded {name}: {e}")),
            }
        }
        if let Err(e) = File::open(&self.options.directory).and_then(|f| f.sync_all()) {
            errors.push(format!("cannot sync checkpoint directory: {e}"));
        }
        errors
    }
    fn next_generation(&self) -> Result<u64, String> {
        next_free_generation(
            &self.options.directory,
            self.manifest.as_ref().map_or(0, |m| m.generation),
        )
    }
    /// Route committed records into this directory's sidecar before the walk
    /// commits anything: a fresh walk's empty in-memory sink becomes a sidecar
    /// whose first segment takes the next free generation; a restored sidecar
    /// must already belong to this directory.
    pub(super) fn attach_records<const N: usize>(&self, state: &State<N>) -> Result<(), String> {
        let mut sink = state.records.borrow_mut();
        match &mut *sink {
            RecordSink::Sidecar(sidecar) => {
                if sidecar.directory() != self.options.directory {
                    return Err("record sidecar belongs to another checkpoint directory".into());
                }
            }
            RecordSink::Memory(records) => {
                if self.manifest.as_ref().is_some_and(|m| m.kind == "state") {
                    return Err(
                        "saved walk state must be resumed with its record sidecar, not an in-memory prefix"
                            .into(),
                    );
                }
                let mut sidecar =
                    Sidecar::new(self.options.directory.clone(), self.next_generation()?);
                for record in std::mem::take(records) {
                    sidecar.push(&record)?;
                }
                *sink = RecordSink::Sidecar(sidecar);
            }
        }
        Ok(())
    }
    /// `save_cancellable` for tests that never cancel.
    #[cfg(test)]
    pub(super) fn save<const N: usize>(
        &mut self,
        state: &State<N>,
        inputs: &[Value],
        frontiers: &[Value],
        force: bool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Value>, String> {
        let never = AtomicBool::new(false);
        let kind = if force {
            SaveKind::Forced
        } else {
            SaveKind::Periodic
        };
        self.save_cancellable(state, inputs, frontiers, kind, &never, observer)
    }
    /// Write one generation when the interval elapsed (or the save is not
    /// `Periodic`). The run's `cancellation`, once set, suppresses the
    /// post-save fold of the edge log; it never cuts the pre-save closure
    /// refresh, whose snapshot the generation persists.
    pub(super) fn save_cancellable<const N: usize>(
        &mut self,
        state: &State<N>,
        inputs: &[Value],
        frontiers: &[Value],
        kind: SaveKind,
        cancellation: &AtomicBool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Value>, String> {
        let force = kind != SaveKind::Periodic;
        if !force && self.last.elapsed().as_secs_f64() < self.effective_interval() {
            return Ok(None);
        }
        if state.error.is_some() {
            return Err("refusing to checkpoint a failed publication prefix".into());
        }
        if self.owners.is_empty() {
            return Err("checkpoint owner binding is unavailable".into());
        }
        let stamp = state.change_stamp();
        if self.last_stamp == Some(stamp) {
            // The retained generation already holds this state. Restart the
            // interval so an idle coordinator re-evaluates (and announces the
            // skip) once per effective interval, not on every heartbeat.
            self.last = Instant::now();
            observer(
                json!({"event":"checkpoint_skipped_unchanged","operation":"owner_domain_walk",
                "generation":self.manifest.as_ref().map(|m| m.generation),"forced":force,
                "family_closure_claim":false}),
            );
            return Ok(None);
        }
        // The pre-save scan is part of the save's cost (and of the adaptive
        // interval). As in 102adcc3, the persisted CLOSED bits and closed
        // counts are current even while the run is being cancelled: a paused
        // generation matches the one 102adcc3 writes for the same state. Only
        // short scratch persists the previous snapshot (stale but valid) and
        // keeps the monitor enabled.
        let started = Instant::now();
        state.closure.borrow_mut().refresh_before_save();
        let started_unix_time = unix_time()?;
        let directory = self.options.directory.clone();
        // A sidecar's open segment already reserved this save's generation.
        let generation = match &*state.records.borrow() {
            RecordSink::Sidecar(sidecar) => {
                if sidecar.directory() != directory
                    || self
                        .manifest
                        .as_ref()
                        .is_some_and(|m| sidecar.generation() <= m.generation)
                {
                    return Err(
                        "record sidecar does not belong to this checkpoint generation".into(),
                    );
                }
                sidecar.generation()
            }
            RecordSink::Memory(_) => self.next_generation()?,
        };
        let meta_path = directory.join(Section::Meta.file_name(generation));
        observer(
            json!({"event":"checkpoint_started","operation":"owner_domain_walk",
            "checkpoint_write":{"state":"writing","directory":directory,"generation":generation,"state_path":meta_path,"started_unix_time":started_unix_time},
            "committed_domains":state.published_count(),"contiguous_publication_watermark":state.queue.next,"committed_events":state.events,"family_closure_claim":false}),
        );
        // Seal the records committed since the previous save; the next
        // segment takes the next free generation after this one.
        let seal_started = Instant::now();
        let sealed = match &mut *state.records.borrow_mut() {
            RecordSink::Sidecar(sidecar) => {
                Some(sidecar.seal(generation, next_free_generation(&directory, generation)?)?)
            }
            RecordSink::Memory(_) => None,
        };
        let seal_seconds = seal_started.elapsed().as_secs_f64();
        let identity = Identity {
            arity: N,
            ready: state.ready(),
            semantics: self.semantics,
        };
        let previous = self
            .manifest
            .as_ref()
            .filter(|m| m.kind == "state" && m.arity as usize == N)
            .map(|m| &m.sections);
        let closure_ref = state.closure.borrow();
        let closure: &super::descendant_closure::Tracker = &closure_ref;
        let domains = state.queue.domains.as_slice();
        let records_ref = state.records.borrow();
        let domains_plan = plan(previous.and_then(|s| s.domains.as_ref()), domains.len());
        // Folded edges gave up their insertion order: a retained tiling that
        // ends inside them (another store's) cannot be extended, only re-tiled.
        let edges_plan = plan(
            previous
                .and_then(|s| s.edges.as_ref())
                .filter(|p| p.total >= closure.folded_edge_count() as u64),
            closure.edge_count(),
        );
        let edges_total = edges_plan.first + edges_plan.count;
        // In-memory records (non-sidecar states in tests) are written from RAM.
        let (records, records_plan) = match &*records_ref {
            RecordSink::Memory(records) => (
                records.as_slice(),
                plan(previous.and_then(|s| s.records.as_ref()), records.len()),
            ),
            RecordSink::Sidecar(sidecar) => (
                &[][..],
                Plan {
                    keep: sidecar.closed().to_vec(),
                    first: sidecar.sealed_total(),
                    count: 0,
                },
            ),
        };
        let g2_log = state.queue.delegation.as_ref().and_then(|l| l.g2());
        let anchors_plan =
            g2_log.map(|log| plan(previous.and_then(|s| s.anchors.as_ref()), log.rows.len()));
        let buckets = state.queue.checkpoint_buckets();
        let ledger = state.queue.checkpoint_ledger();
        let ledger_entries = state.queue.delegation.as_ref().map_or(0, |l| l.len());
        let meta = sections::MetaRef {
            counters: [
                state.events,
                state.successors,
                state.conditional,
                state.job_local_reuse_hits,
                state.pre_admitted_orthant_hits,
                state.frontiers,
                state.completed,
                state.native_records,
                state.routed,
                state.route_masks,
                state.initial_domain_count,
                state.initial_entry_domains_inspected,
            ],
            route_joint_support_masks_pruned: state.route_joint_support_masks_pruned,
            queue: state.queue.checkpoint_metadata(),
            closure: closure.counters(),
            details: &state.details,
            refusals: &state.refusals,
            optional: &state.optional,
            progress: sections::ProgressRef {
                metadata: state.checkpoint_progress_metadata(),
                physical_parent: &state.physical_progress,
            },
            parallel: &state.parallel,
            uncommitted: &state.uncommitted,
            inputs,
            input_frontiers: frontiers,
            streams: &state.streams,
        };
        let mut jobs: Vec<(Section, usize, usize, Writer<'_>)> = vec![
            (
                Section::Meta,
                0,
                0,
                Box::new(move |out| sections::write_meta(out, &meta)),
            ),
            (
                Section::Nodes,
                0,
                closure.node_count(),
                Box::new(move |out| sections::write_nodes(out, &identity, closure)),
            ),
            (
                Section::Index,
                0,
                buckets.len(),
                Box::new(move |out| sections::write_index(out, &identity, &buckets)),
            ),
        ];
        if let Some(ledger) = ledger {
            jobs.push((
                Section::Ledger,
                0,
                ledger_entries,
                Box::new(move |out| sections::write_ledger(out, &identity, ledger, ledger_entries)),
            ));
        }
        if domains_plan.count > 0 {
            let first = domains_plan.first;
            jobs.push((
                Section::Domains,
                first,
                domains_plan.count,
                Box::new(move |out| sections::write_domains(out, &identity, domains, first)),
            ));
        }
        if edges_plan.count > 0 {
            let (first, count) = (edges_plan.first, edges_plan.count);
            jobs.push((
                Section::Edges,
                first,
                count,
                Box::new(move |out| sections::write_edges(out, &identity, closure, first, count)),
            ));
        }
        if records_plan.count > 0 {
            let first = records_plan.first;
            jobs.push((
                Section::Records,
                first,
                records_plan.count,
                Box::new(move |out| sections::write_records(out, &records[first..])),
            ));
        }
        if let (Some(log), Some(anchors)) = (g2_log, anchors_plan.as_ref())
            && anchors.count > 0
        {
            let (first, count) = (anchors.first, anchors.count);
            jobs.push((
                Section::Anchors,
                first,
                count,
                Box::new(move |out| sections::write_anchors(out, &identity, log, first, count)),
            ));
        }
        #[cfg(test)]
        let injected = self.fail_section;
        #[cfg(not(test))]
        let injected: Option<Section> = None;
        let mut slots: Vec<Option<Result<Written, String>>> =
            (0..jobs.len()).map(|_| None).collect();
        rayon::scope(|scope| {
            for ((section, first, count, write), slot) in jobs.into_iter().zip(slots.iter_mut()) {
                let file = section.file_name(generation);
                let path = directory.join(&file);
                scope.spawn(move |_| {
                    let started = Instant::now();
                    if injected == Some(section) {
                        *slot = Some(Err(format!("injected {} section failure", section.name())));
                        return;
                    }
                    *slot = Some(write_section(&path, write).map(|(bytes, blake3)| Written {
                        section,
                        file,
                        first,
                        count,
                        bytes,
                        blake3,
                        seconds: started.elapsed().as_secs_f64(),
                    }));
                });
            }
        });
        drop(closure_ref);
        drop(records_ref);
        let mut written = Vec::new();
        for slot in slots {
            written.push(slot.ok_or("checkpoint section writer did not run")??);
        }
        let mut new_sections = Sections::default();
        let mut new_bytes = 0u64;
        let mut section_seconds = json!({});
        if let Some(Some(segment)) = &sealed {
            new_bytes += segment.bytes;
            section_seconds[Section::Records.name()] = json!(seal_seconds);
        }
        for w in &written {
            new_bytes += w.bytes;
            section_seconds[w.section.name()] = json!(w.seconds);
            if !w.section.segmented() {
                *new_sections.plain_mut(w.section) = Some(SectionRef {
                    file: w.file.clone(),
                    bytes: w.bytes,
                    blake3: w.blake3.clone(),
                });
            }
        }
        let segmented_plans = [
            Some((Section::Domains, domains_plan)),
            Some((Section::Edges, edges_plan)),
            Some((Section::Records, records_plan)),
            anchors_plan.map(|plan| (Section::Anchors, plan)),
        ];
        for (section, plan) in segmented_plans.into_iter().flatten() {
            let mut segments = plan.keep;
            if let Some(w) = written.iter().find(|w| w.section == section) {
                segments.push(Segment {
                    generation,
                    file: w.file.clone(),
                    first: w.first as u64,
                    count: w.count as u64,
                    bytes: w.bytes,
                    blake3: w.blake3.clone(),
                });
            }
            *new_sections.segmented_mut(section) = Some(Segmented {
                total: (plan.first + plan.count) as u64,
                segments,
            });
        }
        // The manifest's effective interval reflects this save's duration so
        // far; the event below carries the final figure including publication.
        self.last_save_seconds = started.elapsed().as_secs_f64();
        let executable_first = self
            .manifest
            .as_ref()
            .map_or(self.executable.clone(), |m| m.executable_first.clone());
        let mut manifest = Manifest {
            schema: SCHEMA,
            format: FORMAT.into(),
            kind: "state".into(),
            generation,
            walk_semantics_version: self.semantics,
            arity: N as u32,
            publication_policy: self.policy.clone(),
            request: self.request.clone(),
            owners: self.owners.clone(),
            executable: self.executable.clone(),
            executable_first: executable_first.clone(),
            sections: new_sections,
            metadata: Value::Null,
            amendments: self.amendments.clone(),
        };
        manifest.validate_structure()?;
        let mut metadata = json!({"state":"saved","directory":directory,"generation":generation,"state_path":meta_path,
            "saved_unix_time":unix_time()?,
            "committed_domains":state.published_count(),"pending_domains":state.queue.domains.len()-state.published_count(),
            "contiguous_publication_watermark":state.queue.next,
            "completed_native_inspections":state.completed,"committed_events":state.events,"paused":state.checkpoint_paused,
            "bytes":manifest.total_bytes(),"new_bytes":new_bytes,"section_seconds":section_seconds,
            "started_unix_time":started_unix_time,"save_seconds":started.elapsed().as_secs_f64(),"duration_seconds":started.elapsed().as_secs_f64()});
        merge(&mut metadata, self.identity_metadata(&executable_first));
        if let Some(label) = self.diagnostic_pause {
            metadata["diagnostic_pause"] = json!(label);
        }
        if let Some(reason) = self.stop_reason {
            metadata["stop_reason"] = json!(reason);
        }
        if let Some(activation) = &self.g2_activation {
            metadata["g2_activation"] = activation.clone();
        }
        manifest.metadata = metadata.clone();
        let cleanup_errors = self.publish(manifest)?;
        // Every edge is durable in insertion order now; the log may fold,
        // unless the walk ends after this save (see `SaveKind`) or is being
        // cancelled towards a pause.
        if kind != SaveKind::Final && !cancellation.load(Ordering::Relaxed) {
            state.closure.borrow_mut().persisted(edges_total);
        }
        self.last_save_seconds = started.elapsed().as_secs_f64();
        self.last_stamp = Some(stamp);
        metadata["duration_seconds"] = json!(self.last_save_seconds);
        metadata["save_seconds"] = json!(self.last_save_seconds);
        metadata["saved_unix_time"] = json!(unix_time()?);
        metadata["effective_interval_seconds"] = json!(self.effective_interval());
        if !cleanup_errors.is_empty() {
            metadata["cleanup_errors"] = json!(cleanup_errors);
        }
        if let Some(manifest) = self.manifest.as_mut() {
            manifest.metadata = metadata.clone();
        }
        let mut event = json!({"event":"checkpoint_saved","operation":"owner_domain_walk","checkpoint":metadata,"family_closure_claim":false});
        if !cleanup_errors.is_empty() {
            event["cleanup_errors"] = json!(cleanup_errors);
        }
        Ok(Some(event))
    }
}
/// The request/policy digest a checkpoint of `request` is bound to.
pub(super) fn request_binding(request: &OwnerDomainWalkRequest) -> String {
    binding(request)
}

/// The request digest of an `epoch` walk (current native walk semantics, W2.0 protocol
/// §11.5, A7). Bound: owner selection, queries, limits, publication and
/// semantics, the D-band, Route and query allowances, the frontier policy,
/// and nondefault Epoch lookup, dispatch, publication order and explicit cut.
/// Not digest-bound: workers, inspection workers, the legacy schedule/lookahead,
/// and the aggregate `max_domains` / `max_events` / `max_frontiers`
/// allowances. CP6 separately authenticates the effective cut/window; an
/// explicit requested window must match, omission inherits on restore.
/// Shared by the epoch export and the closure verifier; the
/// CP5 `binding` above is unchanged.
pub(super) fn epoch_request_binding(request: &OwnerDomainWalkRequest) -> String {
    let mut value = json!({"selection":request.matching.selection_json,
        "queries":request.matching.queries_json,
        "limits":super::limits_json(request),
        "reduction":format!("{:?}",request.matching.reduction_limits),
        "publication":"epoch","walk_semantics_version":super::epoch::EPOCH_WALK_SEMANTICS_VERSION,
        "reuse_initial_d_bands":request.reuse_initial_d_bands,
        "route_domain_overcover":request.route_domain_overcover,
        "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
        "max_route_masks":request.max_route_masks,"subdivision":request.apply_subdivision,
        "max_queries":request.matching.max_queries,"max_query_bytes":request.matching.max_query_bytes,
        "frontier_policy":request.frontier_policy.name()});
    add_preferred_subset_binding(&mut value, request);
    add_finite_replay_binding(&mut value, request);
    // AllMiss preserves the original Epoch binding byte-for-byte. The
    // experimental mode is frozen across resume without a new scalar schema.
    if request.epoch_inspector_lookup != super::OwnerDomainWalkEpochInspectorLookup::AllMiss {
        value["epoch_inspector_lookup"] = json!(request.epoch_inspector_lookup.name());
    }
    if request.epoch_rolling {
        value["epoch_rolling"] = json!(true);
    }
    if request.epoch_dispatch != super::OwnerDomainWalkEpochDispatchPolicy::Fifo {
        value["epoch_dispatch"] = json!(request.epoch_dispatch.name());
    }
    if request.epoch_publication_order != super::OwnerDomainWalkEpochPublicationOrder::OldestPrefix
    {
        value["epoch_publication_order"] = json!(request.epoch_publication_order.name());
    }
    // Preserve historical default request bytes. The effective cut and window
    // are also authenticated by the existing CP6 scalar inventory; a public
    // custom cut additionally binds the replay command used by cold checks.
    if let Some(cut) = request.epoch_cut_size.filter(|&cut| cut != 16) {
        value["epoch_cut_size"] = json!(cut);
    }
    if request.epoch_result_escrow_jobs != 0 {
        value["epoch_result_escrow_jobs"] = json!(request.epoch_result_escrow_jobs);
        value["epoch_result_escrow_bytes"] = json!(request.epoch_result_escrow_bytes);
    }
    if request.g2_residual_anchors != super::OwnerDomainWalkG2ResidualAnchors::Off {
        value["g2_residual_anchors"] = json!(request.g2_residual_anchors.name());
    }
    blake3::hash(value.to_string().as_bytes())
        .to_hex()
        .to_string()
}

/// Read-only decode of one CP5 generation for the offline closure verifier:
/// file lengths and digests, section headers and codecs only. No restore
/// validator, index, ledger or dependency tracker is rebuilt; the verifier
/// re-derives everything it relies on from these raw parts.
pub(super) struct RawCheckpoint<const N: usize> {
    pub generation: u64,
    pub request: String,
    pub publication_policy: String,
    pub walk_semantics_version: u32,
    pub executable: String,
    pub owners: Vec<String>,
    pub counters: [usize; 12],
    pub closure: Value,
    pub inputs: Vec<Value>,
    pub input_frontiers: Vec<Value>,
    pub uncommitted: Vec<Value>,
    pub flags: Vec<u8>,
    pub edges: Vec<(u32, u32)>,
    pub domains: Vec<super::queue::CompactDomain<N>>,
    /// Record sidecar segment files in ID-tile order with their line counts.
    pub records: Vec<(PathBuf, usize)>,
    pub verify_seconds: f64,
    /// The rescue amendment chain of the generation (`rescue.rs`).
    pub amendments: Vec<super::rescue::AmendmentRef>,
    /// Frontier details accepted in an uncommitted prefix, by inspection ID
    /// (an A10 stop can fire inside a chunked publication).
    pub pending_frontiers: Vec<(usize, Vec<Value>)>,
    pub g2_activation: Option<Value>,
}

pub(super) fn read_raw<const N: usize>(directory: &Path) -> Result<RawCheckpoint<N>, String> {
    let manifest = manifest::read(&directory.join("latest.json"))?;
    if manifest.kind != "state" {
        return Err("checkpoint generation holds no walk state".into());
    }
    if !crate::application::routed_campaign::storage::compatible_width(manifest.arity as usize, N) {
        return Err("checkpoint coordinate arity".into());
    }
    let verify_seconds = restore::verify_files(directory, &manifest)?;
    let identity = Identity {
        arity: manifest.arity as usize,
        ready: manifest.publication_policy == "ready",
        semantics: manifest.walk_semantics_version,
    };
    let sections = &manifest.sections;
    let plain = |section: Section| {
        sections
            .plain(section)
            .ok_or_else(|| format!("checkpoint is missing the {} section", section.name()))
    };
    let segmented = |section: Section| {
        sections
            .segmented(section)
            .ok_or_else(|| format!("checkpoint is missing the {} section", section.name()))
    };
    let read = |file: &str, bytes: u64| restore::read_section(directory, file, bytes);
    let meta = sections::read_meta(&read(
        &plain(Section::Meta)?.file,
        plain(Section::Meta)?.bytes,
    )?)?;
    let nodes = plain(Section::Nodes)?;
    let flags = sections::read_nodes(&read(&nodes.file, nodes.bytes)?, &identity)?;
    let mut edges = Vec::new();
    for segment in &segmented(Section::Edges)?.segments {
        sections::read_edges(
            &read(&segment.file, segment.bytes)?,
            &identity,
            segment.first as usize,
            segment.count as usize,
            &mut edges,
        )?;
    }
    let mut domains = Vec::new();
    for segment in &segmented(Section::Domains)?.segments {
        sections::read_domains::<N>(
            &read(&segment.file, segment.bytes)?,
            &identity,
            segment.first as usize,
            segment.count as usize,
            &mut domains,
        )?;
    }
    let records = segmented(Section::Records)?
        .segments
        .iter()
        .map(|segment| (directory.join(&segment.file), segment.count as usize))
        .collect();
    let mut pending_frontiers = Vec::new();
    if !meta.details.is_empty() {
        let id = meta
            .streams
            .active
            .map_or(meta.queue.next(), |ticket| ticket.parent);
        pending_frontiers.push((id, meta.details.clone()));
    }
    for (ticket, context) in &meta.streams.parked {
        if !context.frontier_details().is_empty() {
            pending_frontiers.push((ticket.parent, context.frontier_details().to_vec()));
        }
    }
    Ok(RawCheckpoint {
        g2_activation: manifest
            .metadata
            .get("g2_activation")
            .filter(|v| !v.is_null())
            .cloned(),
        generation: manifest.generation,
        request: manifest.request.clone(),
        publication_policy: manifest.publication_policy.clone(),
        walk_semantics_version: manifest.walk_semantics_version,
        executable: manifest.executable.clone(),
        owners: manifest.owners.clone(),
        counters: meta.counters,
        closure: serde_json::to_value(&meta.closure).map_err(|e| e.to_string())?,
        inputs: meta.inputs,
        input_frontiers: meta.input_frontiers,
        uncommitted: meta.uncommitted,
        flags,
        edges,
        domains,
        records,
        verify_seconds,
        amendments: manifest.amendments.clone(),
        pending_frontiers,
    })
}

fn merge(target: &mut Value, extra: Value) {
    if let (Some(target), Some(extra)) = (target.as_object_mut(), extra.as_object()) {
        for (key, value) in extra {
            target.insert(key.clone(), value.clone());
        }
    }
}

#[cfg(test)]
pub(super) use test_support::{round_trip_state, round_trip_state_on_disk};

#[cfg(test)]
mod tests {
    use super::super::{
        OwnerDomainMatchRequest,
        delegation::{NativeOutcome, SchedulingPolicy},
        queue::{Domain, Phase, Queue},
    };
    use super::test_support::{Fixture, OWNER, test_directory};
    use super::*;
    use std::cell::RefCell;
    use std::time::Duration;

    #[test]
    fn epoch_lookup_control_does_not_change_cp5_binding() {
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "selection".into(),
            "queries".into(),
        ));
        let old = binding(&request);
        request.epoch_inspector_lookup =
            super::super::OwnerDomainWalkEpochInspectorLookup::Snapshot;
        // Native admission refuses this combination; CP5's byte identity is untouched.
        assert_eq!(binding(&request), old);
    }

    fn request(path: &Path) -> OwnerDomainWalkRequest {
        let mut request = OwnerDomainWalkRequest::new(OwnerDomainMatchRequest::new(
            "selection".into(),
            "queries".into(),
        ));
        request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(path));
        request
    }
    fn domain(lo: u64, hi: u64) -> Domain<1> {
        Domain {
            phase: Phase::Apply,
            owner: [true],
            lower: vec![lo],
            upper: vec![Some(hi)],
            rank: None,
            powers: Default::default(),
        }
    }

    #[test]
    fn g2_rescue_chain_base_authenticates_actual_before_after_and_preserves_only_recorded_base() {
        let mut on = request(Path::new("unused-checkpoint"));
        let before = binding(&on);
        on.g2_residual_anchors = super::super::OwnerDomainWalkG2ResidualAnchors::Union;
        let after = binding(&on);
        let receipt = json!({"from":"off", "to":"union",
            "binding_before":before, "binding_after":after});
        let mut recorded = vec![super::super::rescue::AmendmentRef {
            sequence: 1,
            digest: "d".repeat(64),
            parent: before.clone(),
            queries: 1,
            first_input: 1,
            first_domain: 1,
            quarantined: 1,
            resumed_generation: 1,
        }];
        assert_eq!(
            rescue_chain_base(&on, &after, Some(&receipt), &recorded).unwrap(),
            before
        );
        assert_eq!(
            rescue_chain_base(&on, &after, Some(&receipt), &[]).unwrap(),
            after
        );
        recorded[0].parent = after.clone();
        assert_eq!(
            rescue_chain_base(&on, &after, Some(&receipt), &recorded).unwrap(),
            after
        );
        for key in ["binding_before", "binding_after", "from", "to"] {
            let mut corrupt = receipt.clone();
            corrupt[key] = json!("changed");
            assert!(
                rescue_chain_base(&on, &after, Some(&corrupt), &recorded).is_err(),
                "{key}"
            );
        }
        for key in ["binding_before", "binding_after"] {
            let mut corrupt = receipt.clone();
            corrupt.as_object_mut().unwrap().remove(key);
            assert!(rescue_chain_base(&on, &after, Some(&corrupt), &[]).is_err());
        }
        let mut changed = on.clone();
        changed.matching.queries_json = "different exact query roles or bounds".into();
        assert!(
            rescue_chain_base(&changed, &binding(&changed), Some(&receipt), &recorded).is_err()
        );
        assert!(rescue_chain_base(&on, &after, Some(&json!(false)), &recorded).is_err());
    }
    /// Ledger, retirement alias, dependency edges and records: every section
    /// non-empty and mutually consistent, so it resumes.
    fn ledger_fixture() -> State<1> {
        let mut queue = Queue::with_policy(
            100,
            None,
            SchedulingPolicy::TransferUnreserved {
                lookahead: std::num::NonZeroUsize::new(1).unwrap(),
            },
        )
        .unwrap();
        queue.admit(domain(0, 0)).unwrap();
        queue.admit(domain(3, 3)).unwrap();
        queue.admit(domain(2, 4)).unwrap();
        assert_eq!(queue.containment_retired_candidates, 1);
        let ledger = queue.delegation.as_mut().unwrap();
        ledger.native_started(0).unwrap();
        ledger
            .publish_native(
                0,
                NativeOutcome::Completed {
                    unresolved_frontiers: 0,
                },
            )
            .unwrap();
        ledger.publish_delegated(1).unwrap();
        ledger.native_started(2).unwrap();
        queue.next = 2;
        let mut state = State::new(queue, 0, None);
        state.completed = 1;
        state.native_records = 1;
        state.events = 7;
        state.initial_domain_count = 3;
        state.initial_entry_domains_inspected = 1;
        state.closure.borrow_mut().finish(0, true, true);
        state.closure.borrow_mut().edge(1, 2);
        state.closure.borrow_mut().finish(1, false, true);
        let records = state.records.get_mut();
        records
            .push(json!({"id":0,"local_inspection_finished":true,"error":null,"frontiers":[]}))
            .unwrap();
        records
            .push(json!({"id":1,"record_kind":"delegated_not_inspected","representative_id":2}))
            .unwrap();
        state.details.push(json!({"accepted_frontier":3}));
        state
    }

    /// Resume through a store that stays open, keeping the restored record
    /// sidecar (`Fixture::resume` reads it back into RAM instead).
    fn resumed_with_sidecar(fixture: &Fixture) -> (Store, State<1>) {
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        let state = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
        store.attach_records(&state).unwrap();
        (store, state)
    }
    /// Publish `ledger_fixture`'s representative 2 as the walk would.
    fn publish_representative(state: &mut State<1>) {
        let ledger = state.queue.delegation.as_mut().unwrap();
        ledger.native_started(2).unwrap();
        ledger
            .publish_native(
                2,
                NativeOutcome::Completed {
                    unresolved_frontiers: 0,
                },
            )
            .unwrap();
        state.queue.next = ledger.cursor();
        state.completed += 1;
        state.native_records += 1;
        state.closure.borrow_mut().finish(2, true, true);
        state
            .records
            .get_mut()
            .push(json!({"id":2,"local_inspection_finished":true,"error":null,"frontiers":[]}))
            .unwrap();
    }
    fn record_ids(fixture: &Fixture) -> Vec<u64> {
        let state = fixture.resume::<1>().unwrap();
        let rows = state.records.borrow().snapshot();
        rows.iter().map(|r| r["id"].as_u64().unwrap()).collect()
    }

    #[test]
    fn crashed_sidecar_tail_is_skipped_on_resume_and_removed_by_cleanup() {
        let fixture = Fixture::save(&ledger_fixture()); // generation 2
        let (store, mut state) = resumed_with_sidecar(&fixture);
        publish_representative(&mut state);
        let orphan = fixture.dir.join(Section::Records.file_name(3));
        assert!(orphan.exists());
        drop((store, state)); // Crash: generation 3 is never saved.
        let (mut store, mut state) = resumed_with_sidecar(&fixture);
        assert_eq!(state.records.borrow().total(), 2);
        publish_representative(&mut state);
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 4); // 3 is the orphan's.
        assert!(orphan.exists()); // Not yet below the previous generation.
        state.events += 1;
        store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert!(!orphan.exists());
        drop(store);
        assert_eq!(record_ids(&fixture), [0, 1, 2]);
    }

    #[test]
    fn failed_save_after_sealing_lists_the_segment_in_the_next_generation() {
        let fixture = Fixture::save(&ledger_fixture());
        let (mut store, mut state) = resumed_with_sidecar(&fixture);
        publish_representative(&mut state);
        store.fail_section = Some(Section::Index);
        let error = store.save(&state, &[], &[], true, &|_| {}).unwrap_err();
        assert!(error.contains("injected index section failure"), "{error}");
        assert_eq!(fixture.manifest()["generation"], 2);
        assert!(fixture.dir.join(Section::Records.file_name(3)).exists());
        store.fail_section = None;
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 4);
        let manifest = fixture.manifest();
        assert_eq!(manifest["sections"]["records"]["total"], 3);
        assert_eq!(
            manifest["sections"]["records"]["segments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["generation"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [2, 3]
        );
        drop(store);
        assert_eq!(record_ids(&fixture), [0, 1, 2]);
    }

    #[test]
    fn application_refinement_is_exact_checkpoint_policy() {
        use rustred::solver::OwnerAppliedCellRefinement;
        let path = test_directory();
        let mut request = request(&path);
        let off = binding(&request);
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        assert_eq!(binding(&request), off);
        let resumed = Store::open(&request).unwrap().unwrap();
        drop(resumed);
        request.applied_limits.cell_refinement = OwnerAppliedCellRefinement::SingleFiniteAxis {
            max_cardinality: std::num::NonZeroUsize::new(2).unwrap(),
        };
        assert_ne!(binding(&request), off);
        assert!(Store::open(&request).is_err());
        // A new On checkpoint binds the full threshold, not just an enabled bit.
        fs::remove_dir_all(&path).unwrap();
        fs::create_dir(&path).unwrap();
        request.checkpoint.as_mut().unwrap().resume = false;
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        drop(Store::open(&request).unwrap().unwrap());
        request.applied_limits.cell_refinement = OwnerAppliedCellRefinement::SingleFiniteAxis {
            max_cardinality: std::num::NonZeroUsize::new(3).unwrap(),
        };
        assert!(Store::open(&request).is_err());
        request.applied_limits.cell_refinement = OwnerAppliedCellRefinement::Off;
        assert!(Store::open(&request).is_err());
        fs::remove_dir_all(path).unwrap();
    }
    /// A10: Record keeps the historical request value (the pre-A10 formula,
    /// restated here, hashes to the same digest), so every existing CP5
    /// checkpoint and control resumes; Stop is bound and refuses Record.
    #[test]
    fn frontier_stop_is_bound_and_record_keeps_the_historical_binding() {
        let path = test_directory();
        let mut request = request(&path);
        request.disable_work_limits();
        request.workers = 6;
        request.route_domain_overcover = true;
        let historical = |request: &OwnerDomainWalkRequest| {
            let value = json!({"selection":request.matching.selection_json,"queries":request.matching.queries_json,
                "limits":super::super::limits_json(request),"reduction":format!("{:?}",request.matching.reduction_limits),
                "workers":request.workers,"inspection_workers":request.inspection_workers,
                "publication":format!("{:?}",request.publication_policy),"scheduling":format!("{:?}",request.scheduling_policy),
                "reuse_initial_d_bands":request.reuse_initial_d_bands,"max_domains":request.max_domains,
                "max_events":request.max_events,"max_frontiers":request.max_frontiers,
                "max_containment_checks":request.max_containment_checks,"route_domain_overcover":request.route_domain_overcover,
                "route_joint_source_support_pruning":request.route_joint_source_support_pruning,
                "max_route_masks":request.max_route_masks,"subdivision":request.apply_subdivision,
                "max_queries":request.matching.max_queries,"max_query_bytes":request.matching.max_query_bytes});
            blake3::hash(value.to_string().as_bytes())
                .to_hex()
                .to_string()
        };
        assert_eq!(
            request.frontier_policy,
            OwnerDomainWalkFrontierPolicy::Record
        );
        assert_eq!(binding(&request), historical(&request));
        assert!(binding_value(&request).get("frontier_policy").is_none());
        let record = binding(&request);
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
        assert_eq!(binding_value(&request)["frontier_policy"], "stop");
        assert_ne!(binding(&request), record);
        request.checkpoint.as_mut().unwrap().resume = true;
        assert!(Store::open(&request).is_err());
        request.frontier_policy = OwnerDomainWalkFrontierPolicy::Record;
        drop(Store::open(&request).unwrap().unwrap());
        // A Stop checkpoint refuses a Record resume.
        fs::remove_dir_all(&path).unwrap();
        fs::create_dir(&path).unwrap();
        request.checkpoint.as_mut().unwrap().resume = false;
        request.frontier_policy = OwnerDomainWalkFrontierPolicy::Stop;
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        drop(Store::open(&request).unwrap().unwrap());
        request.frontier_policy = OwnerDomainWalkFrontierPolicy::Record;
        assert!(Store::open(&request).is_err());
        fs::remove_dir_all(path).unwrap();
    }
    /// G2' union is bound (and refuses an off resume); off adds no key, so
    /// the binding of every existing request is unchanged.
    #[test]
    fn g2_residual_anchor_policy_is_checkpoint_bound_and_off_is_historical() {
        let path = test_directory();
        let mut request = request(&path);
        let off = binding(&request);
        assert!(binding_value(&request).get("g2_residual_anchors").is_none());
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        request.g2_residual_anchors = super::super::OwnerDomainWalkG2ResidualAnchors::Union;
        assert_ne!(binding(&request), off);
        assert_eq!(binding_value(&request)["g2_residual_anchors"], "union");
        assert!(Store::open(&request).is_err());
        request.g2_residual_anchors = super::super::OwnerDomainWalkG2ResidualAnchors::Off;
        assert_eq!(binding(&request), off);
        drop(Store::open(&request).unwrap().unwrap());
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn joint_source_support_policy_is_checkpoint_bound() {
        let path = test_directory();
        let mut request = request(&path);
        request.route_domain_overcover = true;
        let off = binding(&request);
        let mut store = Store::open(&request).unwrap().unwrap();
        store.bootstrap().unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        drop(Store::open(&request).unwrap().unwrap());
        request.route_joint_source_support_pruning = true;
        assert_ne!(binding(&request), off);
        assert!(Store::open(&request).is_err());
        request.route_joint_source_support_pruning = false;
        drop(Store::open(&request).unwrap().unwrap());
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn joint_support_mask_counter_survives_checkpoint_round_trip() {
        let mut state = State::<1>::new(Queue::new(8, None), 0, None);
        state.route_masks = 21;
        state.route_joint_support_masks_pruned = 7;
        let restored = round_trip_state(&state).unwrap();
        assert_eq!(restored.route_masks, 21);
        assert_eq!(restored.route_joint_support_masks_pruned, 7);
    }

    #[test]
    fn round_trip_preserves_retirement_aliases_and_restarts_only_unfinished_native() {
        let mut state = ledger_fixture();
        let fixture = Fixture::save_with(
            &state,
            Fixture::request_for(&state),
            &[json!({"domain":0})],
            &[],
        );
        let mut restored: Restored<1> = fixture.resume_full().unwrap();
        assert_eq!(restored.state.queue.next, 2);
        assert_eq!(restored.state.completed, 1);
        assert_eq!(restored.state.events, 7);
        assert_eq!(restored.state.details, state.details);
        assert_eq!(
            restored.state.records.borrow().snapshot(),
            state.records.borrow().snapshot()
        );
        assert_eq!(restored.inputs, vec![json!({"domain":0})]);
        assert_eq!(restored.report["dependency_edges"], 1);
        // Started was not completed/cancelled; its original responsibility is
        // Reserved again and can be dispatched exactly once on restart.
        restored
            .state
            .queue
            .delegation
            .as_mut()
            .unwrap()
            .native_started(2)
            .unwrap();
        assert!(
            restored
                .state
                .queue
                .delegation
                .as_mut()
                .unwrap()
                .native_started(0)
                .is_err()
        );
        for candidate in [domain(3, 3), domain(3, 4), domain(1, 5), domain(0, 6)] {
            assert_eq!(
                state.queue.admit(candidate.clone()),
                restored.state.queue.admit(candidate)
            );
            assert_eq!(
                state.queue.containment_checks,
                restored.state.queue.containment_checks
            );
            assert_eq!(
                state.queue.containment_retired_candidates,
                restored.state.queue.containment_retired_candidates
            );
        }
    }

    #[test]
    fn bootstrap_atomic_generations_owner_binding_and_corruption_rejection() {
        let path = test_directory();
        let mut request = request(&path);
        let mut store = Store::open(&request).unwrap().unwrap();
        assert_eq!(request.checkpoint.as_ref().unwrap().interval_seconds, 3600);
        store.bootstrap().unwrap();
        assert_eq!(store.metadata().unwrap()["bootstrap"], true);
        assert_eq!(store.metadata().unwrap()["preparation_must_restart"], true);
        assert!(Store::open(&request).is_err());
        store
            .bind_owners(vec!["actual payload digest".into()])
            .unwrap();
        let mut state = State::<1>::new(Queue::new(8, None), 0, None);
        for step in 0..3 {
            state.events = step; // Each generation carries a different stamp.
            store.save(&state, &[], &[], true, &|_| {}).unwrap();
        }
        let meta = |generation: u64| path.join(Section::Meta.file_name(generation));
        assert!(!meta(1).exists());
        assert!(!meta(2).exists());
        assert!(meta(3).exists());
        assert!(meta(4).exists());
        assert!(!path.join(Section::Nodes.file_name(2)).exists());
        let previous = manifest::read(&path.join("previous.json")).unwrap();
        assert_eq!(previous.generation, 3);
        assert_eq!(previous.kind, "state");
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        let mut resumed = Store::open(&request).unwrap().unwrap();
        assert!(resumed.bind_owners(vec!["changed payload".into()]).is_err());
        resumed
            .bind_owners(vec!["actual payload digest".into()])
            .unwrap();
        assert_eq!(
            resumed.resume::<1>(&|_| {}).unwrap().unwrap().state.events,
            2
        );
        drop(resumed);
        request.max_events += 1;
        assert!(
            Store::open(&request)
                .err()
                .unwrap()
                .contains("request or policy differs")
        );
        request.max_events -= 1;
        OpenOptions::new()
            .append(true)
            .open(path.join(Section::Nodes.file_name(4)))
            .unwrap()
            .write_all(b"corruption")
            .unwrap();
        assert!(
            Store::open(&request)
                .err()
                .unwrap()
                .contains("checksum or length")
        );
        // Unique test-owned directory, never a caller-supplied campaign path.
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn resume_accepts_changed_executable_digest_with_same_semantics_version() {
        let path = test_directory();
        let mut request = request(&path);
        // The saved state carries a lookahead-1 ledger; the request must agree.
        request.scheduling_policy = SchedulingPolicy::TransferUnreserved {
            lookahead: std::num::NonZeroUsize::new(1).unwrap(),
        };
        let mut store = Store::open_with_identity(&request, "exe-a".into(), 1)
            .unwrap()
            .unwrap();
        store.bootstrap().unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        let state = ledger_fixture();
        store.save(&state, &[], &[], true, &|_| {}).unwrap();
        assert_eq!(store.metadata().unwrap()["executable"], "exe-a");
        assert_eq!(
            store.metadata().unwrap()["executable_changed_since_bootstrap"],
            false
        );
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        let mut resumed = Store::open_with_identity(&request, "exe-b".into(), 1)
            .unwrap()
            .unwrap();
        let events = resumed.take_open_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["event"], "checkpoint_executable_changed");
        assert_eq!(events[0]["saved"], "exe-a");
        assert_eq!(events[0]["current"], "exe-b");
        assert!(resumed.take_open_events().is_empty());
        resumed.bind_owners(vec![OWNER.into()]).unwrap();
        let seen = RefCell::new(Vec::new());
        let restored = resumed
            .resume::<1>(&|event| seen.borrow_mut().push(event))
            .unwrap()
            .unwrap();
        assert_eq!(seen.borrow().len(), 1);
        assert_eq!(seen.borrow()[0]["event"], "checkpoint_restored");
        assert_eq!(seen.borrow()[0]["restore"]["generation"], 2);
        assert_eq!(
            seen.borrow()[0]["restore"]["executable_changed_since_bootstrap"],
            true
        );
        assert_eq!(restored.state.events, 7);
        let mut state = restored.state;
        state.events += 1;
        let saved = resumed
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["executable"], "exe-b");
        assert_eq!(
            saved["checkpoint"]["executable_changed_since_bootstrap"],
            true
        );
        drop(resumed);
        let manifest = manifest::read(&path.join("latest.json")).unwrap();
        assert_eq!(manifest.executable, "exe-b");
        assert_eq!(manifest.executable_first, "exe-a");
        assert_eq!(manifest.walk_semantics_version, 1);
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn resume_refuses_changed_semantics_version_even_with_same_executable() {
        let path = test_directory();
        let mut request = request(&path);
        let mut store = Store::open_with_identity(&request, "exe-a".into(), 1)
            .unwrap()
            .unwrap();
        store.bootstrap().unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        store
            .save(
                &State::<1>::new(Queue::new(8, None), 0, None),
                &[],
                &[],
                true,
                &|_| {},
            )
            .unwrap();
        drop(store);
        request.checkpoint.as_mut().unwrap().resume = true;
        let error = Store::open_with_identity(&request, "exe-a".into(), 2)
            .err()
            .unwrap();
        assert!(
            error.contains("walk semantics version differs (saved 1, executable 2)"),
            "{error}"
        );
        assert!(
            Store::open_with_identity(&request, "exe-a".into(), 1)
                .unwrap()
                .is_some()
        );
        fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn ledger_section_presence_is_bound_to_the_scheduling_policy() {
        const REFUSED: &str = "checkpoint request or policy differs; refusing to restart";
        let fixture = Fixture::save(&ledger_fixture());
        let good = fixture.manifest();
        assert!(!good["sections"]["ledger"].is_null());
        // Dropping the optional key passes every digest; the request refuses it.
        let mut tampered = good.clone();
        tampered["sections"]
            .as_object_mut()
            .unwrap()
            .remove("ledger");
        fixture.write_manifest(&tampered);
        assert_eq!(fixture.open(true).err().unwrap(), REFUSED);
        fixture.write_manifest(&good);
        fixture.resume::<1>().unwrap();
        // Mirror: an InspectAll campaign must not acquire a ledger section.
        let plain = Fixture::save(&State::<1>::new(Queue::new(8, None), 0, None));
        let good_plain = plain.manifest();
        let mut tampered = good_plain.clone();
        tampered["sections"]["ledger"] = good["sections"]["ledger"].clone();
        plain.write_manifest(&tampered);
        assert_eq!(plain.open(true).err().unwrap(), REFUSED);
        plain.write_manifest(&good_plain);
        plain.resume::<1>().unwrap();
        // The restored ledger's lookahead is asserted even when digests agree.
        fixture.rewrite_section::<1>(Section::Ledger, |ledger| ledger["lookahead"] = json!(2));
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("does not match the scheduling policy")
        );
        fixture.rewrite_section::<1>(Section::Ledger, |ledger| ledger["lookahead"] = json!(1));
        fixture.resume::<1>().unwrap();
    }

    #[test]
    fn forced_save_after_resume_is_skipped_when_nothing_changed() {
        let state = ledger_fixture();
        let fixture = Fixture::save(&state);
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        let mut resumed = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
        let seen = RefCell::new(Vec::new());
        let observe = |event: Value| seen.borrow_mut().push(event);
        assert!(
            store
                .save(&resumed, &[], &[], true, &observe)
                .unwrap()
                .is_none()
        );
        assert_eq!(seen.borrow().len(), 1);
        assert_eq!(seen.borrow()[0]["event"], "checkpoint_skipped_unchanged");
        assert_eq!(seen.borrow()[0]["generation"], 2);
        assert_eq!(fixture.manifest()["generation"], 2);
        resumed.queue.admit(domain(7, 7)).unwrap();
        resumed.closure.borrow_mut().discovered(4);
        let saved = store
            .save(&resumed, &[], &[], true, &observe)
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 3);
        assert_eq!(fixture.manifest()["sections"]["domains"]["total"], 4);
        assert_eq!(
            fixture.manifest()["sections"]["domains"]["segments"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        // The same stamp again: skipped, even when forced.
        assert!(
            store
                .save(&resumed, &[], &[], true, &observe)
                .unwrap()
                .is_none()
        );
        // A cooperative stop's interrupted-inspection receipt is state too.
        resumed
            .uncommitted
            .push(json!({"id":3,"committed":false,"error":"cancelled"}));
        let saved = store
            .save(&resumed, &[], &[], true, &observe)
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 4);
        drop(store); // Release the directory lock before reopening.
        assert_eq!(fixture.resume::<1>().unwrap().uncommitted.len(), 1);
    }

    #[test]
    fn completed_physical_part_receipt_changes_the_stamp() {
        let state = State::<1>::new(Queue::new(8, None), 0, None);
        let fixture = Fixture::save(&state);
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        let mut resumed = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
        assert!(
            store
                .save(&resumed, &[], &[], true, &|_| {})
                .unwrap()
                .is_none()
        );
        // Part 0 of a subdivided parent finished without accepting an event.
        resumed.physical_progress = Some(super::super::physical_parts::Progress {
            parent: resumed.queue.next,
            completed: vec![json!({"part":0,"stats":{},"seconds":0.5,"error":null})],
        });
        let saved = store
            .save(&resumed, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 3);
        drop(store);
        let again = fixture.resume::<1>().unwrap();
        assert_eq!(again.physical_progress.as_ref().unwrap().completed.len(), 1);
    }

    #[test]
    fn pause_flag_transitions_write_exactly_one_generation() {
        let mut state = ledger_fixture();
        let fixture = Fixture::save(&state);
        assert_eq!(fixture.manifest()["metadata"]["paused"], false);
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        drop(store.resume::<1>(&|_| {}).unwrap().unwrap());
        // A cooperative stop with no in-flight work still records the pause.
        state.checkpoint_paused = true;
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 3);
        assert_eq!(saved["checkpoint"]["paused"], true);
        assert!(
            store
                .save(&state, &[], &[], true, &|_| {})
                .unwrap()
                .is_none()
        );
        drop(store);
        assert_eq!(fixture.manifest()["metadata"]["paused"], true);
        // Resuming the paused checkpoint flips the flag with one write.
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        let resumed = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
        assert!(!resumed.checkpoint_paused);
        let saved = store
            .save(&resumed, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 4);
        assert_eq!(saved["checkpoint"]["paused"], false);
        assert!(
            store
                .save(&resumed, &[], &[], true, &|_| {})
                .unwrap()
                .is_none()
        );
        drop(store);
        assert_eq!(fixture.manifest()["metadata"]["paused"], false);
    }

    #[test]
    fn diagnostic_pause_label_is_optional_metadata_and_never_inherited() {
        let mut state = ledger_fixture();
        let fixture = Fixture::save(&state);
        // Ordinary saves, like every manifest older binaries wrote, carry no label.
        assert!(
            fixture.manifest()["metadata"]
                .get("diagnostic_pause")
                .is_none()
        );
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        drop(store.resume::<1>(&|_| {}).unwrap().unwrap());
        store.mark_diagnostic_pause("ready-multi-prefix");
        state.checkpoint_paused = true;
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(
            saved["checkpoint"]["diagnostic_pause"],
            "ready-multi-prefix"
        );
        drop(store);
        assert_eq!(
            fixture.manifest()["metadata"]["diagnostic_pause"],
            "ready-multi-prefix"
        );
        // A labelled manifest restores; the resuming session saves unlabelled.
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        let resumed = store.resume::<1>(&|_| {}).unwrap().unwrap().state;
        let saved = store
            .save(&resumed, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert!(saved["checkpoint"].get("diagnostic_pause").is_none());
        drop(store);
        assert!(
            fixture.manifest()["metadata"]
                .get("diagnostic_pause")
                .is_none()
        );
    }

    #[test]
    fn unchanged_skip_is_announced_once_per_effective_interval() {
        let state = ledger_fixture();
        let fixture = Fixture::save(&state);
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        store.options.interval_seconds = 3600;
        // Restore fixes the stamp; the interval has elapsed with a stable state.
        drop(store.resume::<1>(&|_| {}).unwrap().unwrap());
        store.last = Instant::now() - Duration::from_secs(7200);
        let seen = RefCell::new(Vec::new());
        let observe = |event: Value| seen.borrow_mut().push(event);
        for _ in 0..3 {
            assert!(
                store
                    .save(&state, &[], &[], false, &observe)
                    .unwrap()
                    .is_none()
            );
        }
        assert_eq!(seen.borrow().len(), 1, "{:?}", seen.borrow());
        assert_eq!(seen.borrow()[0]["event"], "checkpoint_skipped_unchanged");
        assert!(store.last.elapsed() < Duration::from_secs(60));
        assert_eq!(fixture.manifest()["generation"], 2);
        // Once the interval elapses again the skip is re-evaluated once more.
        store.last = Instant::now() - Duration::from_secs(7200);
        assert!(
            store
                .save(&state, &[], &[], false, &observe)
                .unwrap()
                .is_none()
        );
        assert_eq!(seen.borrow().len(), 2);
    }

    #[test]
    fn effective_interval_stretches_after_slow_save() {
        let mut state = ledger_fixture();
        let fixture = Fixture::save(&state);
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        store.options.interval_seconds = 1;
        state.events += 1;
        store.last_save_seconds = 100.0;
        store.last = Instant::now() - Duration::from_secs(5);
        assert!(
            store
                .save(&state, &[], &[], false, &|_| {})
                .unwrap()
                .is_none()
        );
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert!(
            saved["checkpoint"]["effective_interval_seconds"]
                .as_f64()
                .unwrap()
                >= 1.0
        );
        assert!(saved["checkpoint"]["save_seconds"].as_f64().unwrap() < 20.0);
        state.events += 1;
        store.last_save_seconds = 0.0;
        store.last = Instant::now() - Duration::from_secs(5);
        assert_eq!(store.effective_interval(), 1.0);
        assert!(
            store
                .save(&state, &[], &[], false, &|_| {})
                .unwrap()
                .is_some()
        );
        state.events += 1;
        assert!(
            store
                .save(&state, &[], &[], false, &|_| {})
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn every_section_digest_is_verified_before_decoding() {
        let fixture = Fixture::save(&ledger_fixture());
        let manifest = fixture.manifest();
        for section in Section::ALL {
            if section == Section::Anchors {
                // Present only with G2' residual anchors (absent key, not null).
                assert!(manifest["sections"].get("anchors").is_none());
                continue;
            }
            assert!(
                !manifest["sections"][section.name()].is_null(),
                "{}",
                section.name()
            );
            let offset = if section.segmented() || section == Section::Meta {
                2
            } else {
                sections::HEADER_BYTES + 1
            };
            fixture.flip_byte(section, offset);
            let error = fixture.open(true).err().unwrap();
            assert!(
                error.contains("checksum or length"),
                "{}: {error}",
                section.name()
            );
            fixture.flip_byte(section, offset);
            fixture.resume::<1>().unwrap();
        }
    }

    #[test]
    fn segments_must_tile_the_total_without_gaps() {
        let mut state = ledger_fixture();
        let fixture = Fixture::save(&state);
        state.queue.admit(domain(9, 9)).unwrap();
        state.closure.borrow_mut().discovered(4);
        fixture.save_again(&state).unwrap().unwrap();
        let manifest = fixture.manifest();
        assert_eq!(manifest["sections"]["domains"]["total"], 4);
        assert_eq!(manifest["sections"]["domains"]["segments"][1]["first"], 3);
        for edit in [
            |m: &mut Value| m["sections"]["domains"]["segments"][1]["first"] = json!(4),
            |m: &mut Value| m["sections"]["domains"]["total"] = json!(5),
            |m: &mut Value| m["sections"]["domains"]["segments"][1]["count"] = json!(0),
            |m: &mut Value| {
                let segments = m["sections"]["domains"]["segments"].as_array_mut().unwrap();
                segments.remove(0);
            },
        ] {
            let mut corrupt = manifest.clone();
            edit(&mut corrupt);
            fixture.write_manifest(&corrupt);
            assert!(fixture.open(true).err().unwrap().contains("tile"));
        }
        fixture.write_manifest(&manifest);
        assert_eq!(fixture.resume::<1>().unwrap().queue.domains.len(), 4);
    }

    #[test]
    fn manifest_from_cp4_is_refused_with_fresh_campaign_message() {
        let fixture = Fixture::save(&ledger_fixture());
        let good = fixture.manifest();
        let old = json!({"schema":4,"kind":"state","generation":3,"state_file":"state-00000000000000000003.bin",
            "bytes":10,"digest":"0".repeat(64),"request":good["request"],"executable":"x","owners":[OWNER],"metadata":{}});
        fixture.write_manifest(&old);
        assert_eq!(fixture.open(true).err().unwrap(), manifest::FRESH_CAMPAIGN);
        let mut cp3 = good.clone();
        cp3["schema"] = json!(3);
        fixture.write_manifest(&cp3);
        assert_eq!(fixture.open(true).err().unwrap(), manifest::FRESH_CAMPAIGN);
        let mut format = good.clone();
        format["format"] = json!("RUSTRED-WALK-CP4");
        fixture.write_manifest(&format);
        assert_eq!(fixture.open(true).err().unwrap(), manifest::FRESH_CAMPAIGN);
        fixture.write_manifest(&good);
        fixture.resume::<1>().unwrap();
    }

    #[test]
    fn bucket_order_is_deterministic_bytes() {
        let mut queue = Queue::<3>::new(64, None);
        for owner in [
            [true, false, false],
            [false, true, true],
            [true, true, false],
            [false, false, true],
            [true, true, true],
        ] {
            for phase in [Phase::Apply, Phase::Route] {
                queue
                    .admit(Domain {
                        phase,
                        owner,
                        lower: vec![0; 3],
                        upper: vec![Some(2); 3],
                        rank: Some(1),
                        powers: Default::default(),
                    })
                    .unwrap();
            }
        }
        let mut state = State::new(queue, 0, None);
        let fixture = Fixture::save(&state);
        let first = fixture.manifest();
        state.events += 1;
        fixture.save_again(&state).unwrap().unwrap();
        let second = fixture.manifest();
        assert_eq!(first["generation"], 2);
        assert_eq!(second["generation"], 3);
        assert_eq!(
            first["sections"]["index"]["blake3"],
            second["sections"]["index"]["blake3"]
        );
        assert_eq!(
            first["sections"]["index"]["bytes"],
            second["sections"]["index"]["bytes"]
        );
        assert_eq!(
            first["sections"]["domains"]["segments"],
            second["sections"]["domains"]["segments"]
        );
        let restored = fixture.resume::<3>().unwrap();
        assert_eq!(restored.queue.domains.len(), 10);
    }

    #[test]
    fn cleanup_removes_only_unreferenced_files_below_previous() {
        let mut state = State::<1>::new(Queue::new(64, None), 0, None);
        state.queue.admit(domain(0, 0)).unwrap();
        state.queue.admit(domain(1, 1)).unwrap();
        let fixture = Fixture::save(&state); // generation 2: domains-2 [0,2)
        let stray = fixture.dir.join("notes.txt");
        fs::write(&stray, b"kept").unwrap();
        state.queue.admit(domain(2, 2)).unwrap();
        state.closure.borrow_mut().discovered(3);
        fixture.save_again(&state).unwrap().unwrap(); // 3: domains-3 [2,3)
        state.events += 1;
        fixture.save_again(&state).unwrap().unwrap(); // 4: no new domains
        state.queue.admit(domain(3, 3)).unwrap();
        state.closure.borrow_mut().discovered(4);
        fixture.save_again(&state).unwrap().unwrap(); // 5: domains-5 [3,4)
        let mut names: Vec<String> = fs::read_dir(&fixture.dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        let expect = |section: Section, generation: u64| section.file_name(generation);
        assert_eq!(
            names,
            [
                "checkpoint.lock".to_owned(),
                expect(Section::Domains, 2),
                expect(Section::Domains, 3),
                expect(Section::Domains, 5),
                expect(Section::Index, 4),
                expect(Section::Index, 5),
                "latest.json".to_owned(),
                expect(Section::Meta, 4),
                expect(Section::Meta, 5),
                expect(Section::Nodes, 4),
                expect(Section::Nodes, 5),
                "notes.txt".to_owned(),
                "previous.json".to_owned(),
            ]
        );
        let manifest = fixture.manifest();
        assert_eq!(manifest["sections"]["domains"]["total"], 4);
        assert_eq!(
            manifest["sections"]["domains"]["segments"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s["generation"].as_u64().unwrap())
                .collect::<Vec<_>>(),
            [2, 3, 5]
        );
        assert_eq!(fixture.resume::<1>().unwrap().queue.domains.len(), 4);
    }

    #[test]
    fn failed_save_keeps_old_authority_and_orphans() {
        let mut state = State::<1>::new(Queue::new(64, None), 0, None);
        state.queue.admit(domain(0, 0)).unwrap();
        let fixture = Fixture::save(&state); // generation 2
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        state.queue.admit(domain(1, 1)).unwrap();
        state.closure.borrow_mut().discovered(2);
        store.fail_section = Some(Section::Index);
        let error = store.save(&state, &[], &[], true, &|_| {}).unwrap_err();
        assert!(error.contains("injected index section failure"), "{error}");
        assert_eq!(fixture.manifest()["generation"], 2);
        assert_eq!(
            manifest::read(&fixture.dir.join("previous.json"))
                .unwrap()
                .generation,
            1
        );
        for section in [
            Section::Meta,
            Section::Nodes,
            Section::Index,
            Section::Domains,
        ] {
            assert!(fixture.dir.join(section.file_name(2)).exists());
        }
        assert!(fixture.dir.join(Section::Meta.file_name(3)).exists());
        assert!(fixture.dir.join(Section::Domains.file_name(3)).exists());
        assert!(!fixture.dir.join(Section::Index.file_name(3)).exists());
        assert!(fixture.dir.join(Section::Meta.file_name(1)).exists());
        store.fail_section = None;
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 4); // 3 is taken by orphans.
        assert!(fixture.dir.join(Section::Meta.file_name(3)).exists());
        assert!(!fixture.dir.join(Section::Meta.file_name(1)).exists());
        state.events += 1;
        store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert!(!fixture.dir.join(Section::Meta.file_name(3)).exists());
        assert!(!fixture.dir.join(Section::Domains.file_name(3)).exists());
        drop(store);
        assert_eq!(fixture.resume::<1>().unwrap().queue.domains.len(), 2);
    }

    #[test]
    fn cleanup_failure_after_publish_is_reported_not_fatal() {
        let mut state = State::<1>::new(Queue::new(64, None), 0, None);
        let fixture = Fixture::save(&state); // generation 2; bootstrap meta-1 remains
        let mut store = fixture.open(true).unwrap();
        store.bind_owners(vec![OWNER.into()]).unwrap();
        drop(store.resume::<1>(&|_| {}).unwrap().unwrap());
        store.fail_cleanup = true;
        state.events += 1;
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 3);
        assert!(
            saved["cleanup_errors"][0]
                .as_str()
                .unwrap()
                .contains("injected cleanup failure"),
            "{saved}"
        );
        assert_eq!(
            saved["checkpoint"]["cleanup_errors"],
            saved["cleanup_errors"]
        );
        assert_eq!(fixture.manifest()["generation"], 3);
        assert!(fixture.dir.join(Section::Meta.file_name(1)).exists());
        // The generation is the authority: stamp and interval were updated.
        assert!(
            store
                .save(&state, &[], &[], true, &|_| {})
                .unwrap()
                .is_none()
        );
        store.fail_cleanup = false;
        state.events += 1;
        let saved = store
            .save(&state, &[], &[], true, &|_| {})
            .unwrap()
            .unwrap();
        assert_eq!(saved["checkpoint"]["generation"], 4);
        assert!(saved.get("cleanup_errors").is_none());
        assert!(saved["checkpoint"].get("cleanup_errors").is_none());
        assert!(!fixture.dir.join(Section::Meta.file_name(1)).exists());
        assert!(!fixture.dir.join(Section::Meta.file_name(2)).exists());
        drop(store);
        assert_eq!(fixture.resume::<1>().unwrap().events, 2);
    }

    #[test]
    fn rewritten_sections_fail_semantic_validation_not_checksums() {
        let fixture = Fixture::save(&ledger_fixture());
        fixture.rewrite_section::<1>(Section::Nodes, |flags| flags[2] = json!(1));
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("dependency sealed or inspected an unpublished node")
        );
        fixture.rewrite_section::<1>(Section::Nodes, |flags| flags[2] = json!(0));
        fixture.rewrite_section::<1>(Section::Edges, |edges| edges[0] = json!([1, 3]));
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("invalid checkpoint dependency edge")
        );
        fixture.rewrite_section::<1>(Section::Edges, |edges| edges[0] = json!([1, 2]));
        fixture.rewrite_section::<1>(Section::Meta, |meta| {
            meta["closure"]["inspected"] = json!(2)
        });
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("dependency closure counters")
        );
        fixture.rewrite_section::<1>(Section::Meta, |meta| {
            meta["closure"]["inspected"] = json!(1)
        });
        fixture.rewrite_section::<1>(Section::Ledger, |ledger| ledger["cursor"] = json!(3));
        let error = fixture.resume::<1>().err().unwrap();
        assert!(error.contains("not a contiguous prefix"), "{error}");
        fixture.rewrite_section::<1>(Section::Ledger, |ledger| ledger["cursor"] = json!(2));
        fixture.rewrite_section::<1>(Section::Records, |records| {
            records.as_array_mut().unwrap().pop();
        });
        assert_eq!(
            fixture.resume::<1>().err().unwrap(),
            "checkpoint records/publications disagree"
        );
        fixture.rewrite_section::<1>(Section::Records, |records| {
            records.as_array_mut().unwrap().push(
                json!({"id":1,"record_kind":"delegated_not_inspected","representative_id":2}),
            );
        });
        fixture.rewrite_section::<1>(Section::Domains, |domains| {
            // Duplicates domain 0 exactly; phase, owner, rank and powers agree.
            domains[1]["lower"] = json!([0]);
            domains[1]["upper"] = json!([0]);
        });
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("duplicate checkpoint exact domain")
        );
        fixture.rewrite_section::<1>(Section::Domains, |domains| {
            domains[1]["lower"] = json!([3]);
            domains[1]["upper"] = json!([3]);
        });
        fixture.rewrite_section::<1>(Section::Index, |buckets| {
            buckets[0][2]["orthant"] = json!(5);
        });
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("invalid checkpoint owner bucket")
        );
        fixture.rewrite_section::<1>(Section::Index, |buckets| {
            buckets[0][2]["orthant"] = Value::Null;
        });
        // Each indexed candidate appears once: retiring a repeated ID would
        // release its compact summary slot twice mid-walk. Live candidates 0
        // and 2 sit in different signature groups; list 0 in 2's group too.
        let set_group_of_2 = |buckets: &mut Value, ids: [u64; 2], len: u64| {
            let indexed = &mut buckets[0][2]["indexed"];
            let group = indexed["groups"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|group| {
                    group["blocks"][0]["ids"].as_array().unwrap()[..2].contains(&json!(2))
                })
                .unwrap();
            let old = group["live"].as_u64().unwrap();
            group["blocks"][0]["ids"][0] = json!(ids[0]);
            group["blocks"][0]["ids"][1] = json!(ids[1]);
            group["blocks"][0]["len"] = json!(len);
            group["live"] = json!(len);
            indexed["live"] = json!(indexed["live"].as_u64().unwrap() + len - old);
        };
        fixture.rewrite_section::<1>(Section::Index, |buckets| set_group_of_2(buckets, [0, 2], 2));
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("duplicate checkpoint index ID")
        );
        fixture.rewrite_section::<1>(Section::Index, |buckets| set_group_of_2(buckets, [2, 0], 1));
        // An indexed candidate must belong to its bucket's (phase, owner).
        fixture.rewrite_section::<1>(Section::Domains, |domains| {
            domains[2]["phase"] = json!("Route");
        });
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("invalid checkpoint owner bucket")
        );
        fixture.rewrite_section::<1>(Section::Domains, |domains| {
            domains[2]["phase"] = json!("Apply");
        });
        // A transport record outside the compact queue range (finite
        // coordinates above 65534) is refused explicitly, never truncated.
        fixture.rewrite_section::<1>(Section::Domains, |domains| {
            domains[1]["upper"] = json!([65_535]);
        });
        let error = fixture.resume::<1>().err().unwrap();
        assert!(error.contains("compact queue range"), "{error}");
        fixture.rewrite_section::<1>(Section::Domains, |domains| {
            domains[1]["upper"] = json!([3]);
        });
        fixture.rewrite_bytes(Section::Nodes, |bytes| bytes[12] ^= 1); // semantics
        assert!(
            fixture
                .resume::<1>()
                .err()
                .unwrap()
                .contains("disagrees with the manifest")
        );
        fixture.rewrite_bytes(Section::Nodes, |bytes| bytes[12] ^= 1);
        fixture.rewrite_bytes(Section::Edges, |bytes| bytes.truncate(bytes.len() - 1));
        assert!(fixture.resume::<1>().err().unwrap().contains("length"));
        fixture.rewrite_bytes(Section::Edges, |bytes| bytes.push(0));
        assert_eq!(fixture.resume::<1>().unwrap().events, 7);
    }
}
