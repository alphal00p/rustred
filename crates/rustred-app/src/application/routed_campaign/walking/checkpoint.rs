//! Durable coordinator state, not a completed reduction artifact.
//!
//! CP5 layout (flat, generation-suffixed): `latest.json`/`previous.json` are
//! the manifests; `meta-<G>.json`, `nodes-<G>.bin`, `ledger-<G>.bin` and
//! `index-<G>.bin` are rewritten each generation; `domains-<S>.bin`,
//! `edges-<S>.bin` and `records-<S>.jsonl` are append-only segments that tile
//! their section, so a save costs O(new state) plus the small sections. Every
//! digest is computed while writing; resume verifies every referenced file in
//! parallel before decoding. Resume is bound to the request/policy digest,
//! the owner digests and `WALK_SEMANTICS_VERSION`; the executable digest is
//! recorded and reported, never a refusal.
pub(super) mod manifest;
pub(super) mod restore;
pub(super) mod sections;
#[cfg(test)]
pub(super) mod test_support;

use super::{
    OwnerDomainWalkPublicationPolicy, OwnerDomainWalkRequest, OwnerDomainWalkSchedulingPolicy,
    WALK_SEMANTICS_VERSION,
    execution::{ChangeStamp, State},
};
use crate::application::atomic_file::{write_file_atomically, write_file_atomically_with};
use manifest::{FORMAT, Manifest, SCHEMA, Section, SectionRef, Sections, Segment, Segmented};
pub(super) use restore::Restored;
use sections::{HashingWriter, Identity};
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

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
    #[cfg(test)]
    fail_section: Option<Section>,
}
fn binding(request: &OwnerDomainWalkRequest) -> String {
    // Checkpoint location, interval and resume mode are transport, not policy.
    let value = json!({"selection":request.matching.selection_json,"queries":request.matching.queries_json,
        "limits":super::limits_json(request),"reduction":format!("{:?}",request.matching.reduction_limits),
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
}
fn policy_name(policy: OwnerDomainWalkPublicationPolicy) -> &'static str {
    match policy {
        OwnerDomainWalkPublicationPolicy::Ordered => "ordered",
        OwnerDomainWalkPublicationPolicy::Ready => "ready",
        OwnerDomainWalkPublicationPolicy::OwnerBatched => "owner_batched",
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
        let manifest = if options.resume {
            let m = manifest::read(&options.directory.join("latest.json"))?;
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
            #[cfg(test)]
            fail_section: None,
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
        self.last_stamp = Some(restored.state.change_stamp());
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
    pub(super) fn metadata(&self) -> Option<&Value> {
        self.manifest.as_ref().map(|m| &m.metadata)
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
        };
        self.publish(manifest)?;
        Ok(Some(
            json!({"event":"checkpoint_saved","operation":"owner_domain_walk","checkpoint":metadata,"family_closure_claim":false}),
        ))
    }
    fn publish(&mut self, manifest: Manifest) -> Result<(), String> {
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
        // Only our own validated section names below the previous good
        // generation, and only when neither retained manifest references
        // them, are ours to remove. A failed save returns before this point,
        // keeping both the old authority and any newly written orphan.
        if let Some(previous) = previous {
            let latest = self.manifest.as_ref().expect("published manifest");
            let referenced: HashSet<&str> = latest
                .files()
                .into_iter()
                .chain(previous.files())
                .map(|f| f.file)
                .collect();
            for entry in fs::read_dir(&self.options.directory).map_err(|e| e.to_string())? {
                let entry = entry.map_err(|e| e.to_string())?;
                let name = entry.file_name();
                let name = name.to_string_lossy();
                let Some((_, generation)) = Section::parse(&name) else {
                    continue;
                };
                if generation < previous.generation
                    && !referenced.contains(name.as_ref())
                    && entry.file_type().map_err(|e| e.to_string())?.is_file()
                {
                    fs::remove_file(entry.path()).map_err(|e| {
                        format!("checkpoint saved, old generation cleanup failed: {e}")
                    })?;
                }
            }
            File::open(&self.options.directory)
                .and_then(|f| f.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn next_generation(&self) -> Result<u64, String> {
        let mut generation = self
            .manifest
            .as_ref()
            .map_or(Some(1), |m| m.generation.checked_add(1))
            .ok_or("checkpoint generation overflow")?;
        // Orphans of a failed save keep their generation number to themselves.
        while Section::ALL.iter().any(|s| {
            self.options
                .directory
                .join(s.file_name(generation))
                .exists()
        }) {
            generation = generation
                .checked_add(1)
                .ok_or("checkpoint generation overflow")?;
        }
        Ok(generation)
    }
    pub(super) fn save<const N: usize>(
        &mut self,
        state: &State<N>,
        inputs: &[Value],
        frontiers: &[Value],
        force: bool,
        observer: &impl Fn(Value),
    ) -> Result<Option<Value>, String> {
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
        // Persisted closed counts are current, not a throttled stale snapshot.
        state
            .closure
            .borrow_mut()
            .refresh(&AtomicBool::new(false), true);
        let started = Instant::now();
        let started_unix_time = unix_time()?;
        let generation = self.next_generation()?;
        let directory = self.options.directory.clone();
        let meta_path = directory.join(Section::Meta.file_name(generation));
        observer(
            json!({"event":"checkpoint_started","operation":"owner_domain_walk",
            "checkpoint_write":{"state":"writing","directory":directory,"generation":generation,"state_path":meta_path,"started_unix_time":started_unix_time},
            "committed_domains":state.published_count(),"contiguous_publication_watermark":state.queue.next,"committed_events":state.events,"family_closure_claim":false}),
        );
        let identity = Identity {
            arity: N,
            ready: state.ready(),
            semantics: self.semantics,
        };
        let previous = self
            .manifest
            .as_ref()
            .filter(|m| m.kind == "state")
            .map(|m| &m.sections);
        let closure_ref = state.closure.borrow();
        let closure: &super::descendant_closure::Tracker = &closure_ref;
        let domains = state.queue.domains.as_slice();
        let records = state.records.as_slice();
        let domains_plan = plan(previous.and_then(|s| s.domains.as_ref()), domains.len());
        let edges_plan = plan(
            previous.and_then(|s| s.edges.as_ref()),
            closure.edge_count(),
        );
        let records_plan = plan(previous.and_then(|s| s.records.as_ref()), records.len());
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
        let mut written = Vec::new();
        for slot in slots {
            written.push(slot.ok_or("checkpoint section writer did not run")??);
        }
        let mut new_sections = Sections::default();
        let mut new_bytes = 0u64;
        let mut section_seconds = json!({});
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
        for (section, plan) in [
            (Section::Domains, domains_plan),
            (Section::Edges, edges_plan),
            (Section::Records, records_plan),
        ] {
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
        manifest.metadata = metadata.clone();
        self.publish(manifest)?;
        self.last_save_seconds = started.elapsed().as_secs_f64();
        self.last_stamp = Some(stamp);
        metadata["duration_seconds"] = json!(self.last_save_seconds);
        metadata["save_seconds"] = json!(self.last_save_seconds);
        metadata["saved_unix_time"] = json!(unix_time()?);
        metadata["effective_interval_seconds"] = json!(self.effective_interval());
        if let Some(manifest) = self.manifest.as_mut() {
            manifest.metadata = metadata.clone();
        }
        Ok(Some(
            json!({"event":"checkpoint_saved","operation":"owner_domain_walk","checkpoint":metadata,"family_closure_claim":false}),
        ))
    }
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
        state
            .records
            .push(json!({"id":0,"local_inspection_finished":true,"error":null,"frontiers":[]}));
        state
            .records
            .push(json!({"id":1,"record_kind":"delegated_not_inspected","representative_id":2}));
        state.details.push(json!({"accepted_frontier":3}));
        state
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
        assert_eq!(restored.state.records, state.records);
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
        assert!(fixture.resume::<1>().is_err());
        fixture.rewrite_section::<1>(Section::Ledger, |ledger| ledger["cursor"] = json!(2));
        fixture.rewrite_section::<1>(Section::Records, |records| {
            records.as_array_mut().unwrap().pop();
        });
        assert!(fixture.resume::<1>().is_err());
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
