//! Resume pipeline: every referenced file's length and digest is verified in
//! parallel before any section is decoded; then the same semantic checks as
//! the previous single-file codec run over the restored parts. Records are
//! never loaded: the record sidecar is reconstructed from the manifest's
//! segment list and cross-checked through the ledger and closure state.
use super::super::{
    delegation::G2Log,
    descendant_closure::Tracker,
    execution::{
        PROGRESS_ACCEPTED_EVENTS, State,
        g2::lent_scope,
        records::{self, RecordSink, Sidecar},
    },
    g2::kind,
    queue::{CompactDomain, Domain, Phase, Queue},
    verify_closure::lattice::Cell,
};
use super::manifest::{Manifest, Section};
use super::sections::{self, Identity};
#[cfg(not(target_arch = "wasm32"))]
use rayon::prelude::*;
use serde_json::{Value, json};
use std::fs::File;
use std::io::Read;
use std::ops::ControlFlow;
use std::path::Path;
use std::time::Instant;

/// Wall seconds of each restore phase, in execution order, for the
/// `checkpoint_restored` report (`phase_seconds`). A phase that runs once per
/// segment accumulates.
#[derive(Default)]
pub(in super::super) struct Phases(Vec<(&'static str, f64)>);
impl Phases {
    pub fn since(&mut self, phase: &'static str, started: Instant) {
        let seconds = started.elapsed().as_secs_f64();
        match self.0.iter_mut().find(|(name, _)| *name == phase) {
            Some((_, total)) => *total += seconds,
            None => self.0.push((phase, seconds)),
        }
    }
    pub fn json(&self) -> Value {
        Value::Object(
            self.0
                .iter()
                .map(|&(phase, seconds)| (phase.to_owned(), json!(seconds)))
                .collect(),
        )
    }
}

pub(in super::super) struct Restored<const N: usize> {
    pub state: State<N>,
    pub inputs: Vec<Value>,
    pub input_frontiers: Vec<Value>,
    /// Phase timings and counts for the `checkpoint_restored` event.
    pub report: Value,
}

pub(super) fn file_digest(path: &Path) -> Result<(u64, String), String> {
    let mut f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hash = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
        bytes = bytes
            .checked_add(n as u64)
            .ok_or("checkpoint byte overflow")?;
    }
    Ok((bytes, hash.finalize().to_hex().to_string()))
}

fn section_path(dir: &Path, file: &str) -> Result<std::path::PathBuf, String> {
    let path = dir.join(file);
    if std::fs::symlink_metadata(&path)
        .map_err(|e| format!("{file}: {e}"))?
        .file_type()
        .is_symlink()
    {
        return Err(format!("checkpoint section {file} may not be a symlink"));
    }
    Ok(path)
}

/// Streams every referenced file once, in parallel, retaining nothing.
pub(super) fn verify_files(dir: &Path, manifest: &Manifest) -> Result<f64, String> {
    let started = Instant::now();
    let files = manifest.files();
    #[cfg(not(target_arch = "wasm32"))]
    let files = files.par_iter();
    #[cfg(target_arch = "wasm32")]
    let mut files = files.iter();
    files.try_for_each(|f| {
        let path = section_path(dir, f.file)?;
        if file_digest(&path)? != (f.bytes, f.blake3.to_owned()) {
            return Err(format!(
                "checkpoint state checksum or length mismatch: {}",
                f.file
            ));
        }
        Ok(())
    })?;
    Ok(started.elapsed().as_secs_f64())
}

pub(super) fn read_section(dir: &Path, file: &str, bytes: u64) -> Result<Vec<u8>, String> {
    let path = section_path(dir, file)?;
    let mut out = Vec::new();
    out.try_reserve_exact(usize::try_from(bytes).map_err(|_| "checkpoint section size")?)
        .map_err(|_| "checkpoint section allocation")?;
    File::open(&path)
        .map_err(|e| format!("{file}: {e}"))?
        .take(bytes + 1)
        .read_to_end(&mut out)
        .map_err(|e| format!("{file}: {e}"))?;
    if out.len() as u64 != bytes {
        return Err(format!(
            "checkpoint state checksum or length mismatch: {file}"
        ));
    }
    Ok(out)
}

pub(super) fn restore<const N: usize>(
    dir: &Path,
    manifest: &Manifest,
    verify_seconds: f64,
) -> Result<Restored<N>, String> {
    if manifest.kind != "state" {
        return Err("bootstrap checkpoint has no walk state".into());
    }
    if !crate::application::routed_campaign::storage::compatible_width(manifest.arity as usize, N) {
        return Err("checkpoint coordinate arity".into());
    }
    let ready = manifest.publication_policy == "ready";
    let identity = Identity {
        arity: manifest.arity as usize,
        ready,
        semantics: manifest.walk_semantics_version,
    };
    let decode_started = Instant::now();
    let mut phases = Phases::default();
    let s = &manifest.sections;
    let plain = |section: Section| {
        s.plain(section)
            .ok_or_else(|| format!("state manifest is missing the {} section", section.name()))
    };
    let mut meta = sections::read_meta(&read_section(
        dir,
        &plain(Section::Meta)?.file,
        plain(Section::Meta)?.bytes,
    )?)?;
    phases.since("meta_decode", decode_started);
    let started = Instant::now();
    let nodes = plain(Section::Nodes)?;
    let flags = sections::read_nodes(&read_section(dir, &nodes.file, nodes.bytes)?, &identity)?;
    phases.since("nodes_decode", started);
    let started = Instant::now();
    let edge_sections = s
        .edges
        .as_ref()
        .ok_or("state manifest is missing the edges section")?;
    // One exact allocation: the manifest's segments tile [0, total), so the
    // per-segment reserves below never grow it (amortized growth could leave
    // up to twice the pairs reserved while the CSR is built next to them).
    let mut edges = Vec::new();
    edges
        .try_reserve_exact(
            usize::try_from(edge_sections.total).map_err(|_| "checkpoint segment range")?,
        )
        .map_err(|_| "dependency edge allocation")?;
    for segment in &edge_sections.segments {
        let first = usize::try_from(segment.first).map_err(|_| "checkpoint segment range")?;
        let count = usize::try_from(segment.count).map_err(|_| "checkpoint segment range")?;
        sections::read_edges(
            &read_section(dir, &segment.file, segment.bytes)?,
            &identity,
            first,
            count,
            &mut edges,
        )?;
    }
    phases.since("edges_decode", started);
    let started = Instant::now();
    let domain_segments = &s
        .domains
        .as_ref()
        .ok_or("state manifest is missing the domains section")?
        .segments;
    let mut domains = Vec::new();
    // Room for the whole compact image up front: segment-by-segment growth
    // could leave up to twice the needed capacity. Best effort only; the
    // segment reader reports a genuine allocation failure itself.
    if let Some(total) = domain_segments
        .iter()
        .try_fold(0_u64, |total, s| total.checked_add(s.count))
        .and_then(|total| usize::try_from(total).ok())
    {
        let _ = domains.try_reserve_exact(total);
    }
    for segment in domain_segments {
        let first = usize::try_from(segment.first).map_err(|_| "checkpoint segment range")?;
        let count = usize::try_from(segment.count).map_err(|_| "checkpoint segment range")?;
        sections::read_domains::<N>(
            &read_section(dir, &segment.file, segment.bytes)?,
            &identity,
            first,
            count,
            &mut domains,
        )?;
    }
    phases.since("domains_decode", started);
    let started = Instant::now();
    let ledger = s
        .ledger
        .as_ref()
        .map(|l| sections::read_ledger(&read_section(dir, &l.file, l.bytes)?, &identity))
        .transpose()?;
    phases.since("ledger_decode", started);
    let started = Instant::now();
    let index = plain(Section::Index)?;
    let buckets = sections::read_index(&read_section(dir, &index.file, index.bytes)?, &identity)?;
    phases.since("index_decode", started);
    // The sidecar resumes from the manifest's segments; its next segment takes
    // the next free generation, skipping any orphan of a crash or failed save.
    let sidecar = Sidecar::restored(
        dir.to_path_buf(),
        s.records
            .as_ref()
            .ok_or("state manifest is missing the records section")?
            .segments
            .clone(),
        super::next_free_generation(dir, manifest.generation)?,
    );
    let decode_seconds = decode_started.elapsed().as_secs_f64();
    let validate_started = Instant::now();
    let [
        events,
        successors,
        conditional,
        job_local_reuse_hits,
        pre_admitted_orthant_hits,
        frontiers,
        completed,
        native_records,
        routed,
        route_masks,
        initial_domain_count,
        initial_entry_domains_inspected,
    ] = meta.counters;
    // Amended walks can contain exact duplicates of quarantined domains.
    let mut queue = Queue::<N>::restore_from_parts_amended(
        meta.queue,
        domains,
        buckets,
        ledger,
        !manifest.amendments.is_empty(),
        &mut phases,
    )?;
    if let Some(anchors) = &s.anchors {
        let started = Instant::now();
        let mut log = G2Log::new(meta.counters[10]);
        for segment in &anchors.segments {
            let first = usize::try_from(segment.first).map_err(|_| "checkpoint segment range")?;
            let count = usize::try_from(segment.count).map_err(|_| "checkpoint segment range")?;
            sections::read_anchors(
                &read_section(dir, &segment.file, segment.bytes)?,
                &identity,
                first,
                count,
                &mut log,
            )?;
        }
        if log.rows.len() as u64 != anchors.total {
            return Err("checkpoint G2' log disagrees with its manifest total".into());
        }
        let ledger = queue
            .delegation
            .as_mut()
            .ok_or("checkpoint G2' log without a responsibility ledger")?;
        ledger.attach_g2(log)?;
        phases.since("g2_log_decode", started);
        let started = Instant::now();
        let domains = &queue.domains;
        let ledger = queue.delegation.as_ref().expect("attached");
        ledger.validate_g2(|id| domains.get(id).is_some_and(|d| d.phase() == Phase::Apply))?;
        validate_g2_cover(ledger.g2().expect("attached"), domains)?;
        phases.since("g2_log_validate", started);
    }
    let queue = queue;
    if initial_domain_count > queue.domains.len()
        || meta.route_joint_support_masks_pruned > route_masks
        || initial_entry_domains_inspected > initial_domain_count
        || completed > native_records
        || native_records
            > queue
                .delegation
                .as_ref()
                .map_or(queue.next, |l| l.published_count())
        || ready != queue.delegation.as_ref().is_some_and(|l| l.is_ready())
    {
        return Err("inconsistent checkpoint publication counters".into());
    }
    let saved_accepted_events = meta
        .progress
        .as_object_mut()
        .ok_or("invalid checkpoint progress")?
        .remove(PROGRESS_ACCEPTED_EVENTS)
        .map(|value| {
            value
                .as_u64()
                .and_then(|total| usize::try_from(total).ok())
                .ok_or("invalid ready accepted-events aggregate")
        })
        .transpose()?;
    let derived = ready && saved_accepted_events.is_none();
    let records_accepted_events = match saved_accepted_events {
        Some(total) if ready => total,
        Some(0) | None if !ready => 0,
        Some(_) => return Err("ordered checkpoint carries ready accepted-event accounting".into()),
        // Written by a binary that keeps no aggregate (before the sidecar, or
        // an executable-history rollback that saved again): derive it once.
        None => {
            let started = Instant::now();
            let total = derive_accepted_events(&sidecar, native_records)?;
            phases.since("accepted_events_derivation", started);
            total
        }
    };
    let mut state = State::new(queue, frontiers, None);
    state.records = std::cell::RefCell::new(RecordSink::Sidecar(sidecar));
    state.records_accepted_events = records_accepted_events;
    state.details = meta.details;
    state.refusals = meta.refusals;
    state.optional = meta.optional;
    state.events = events;
    state.successors = successors;
    state.conditional = conditional;
    state.job_local_reuse_hits = job_local_reuse_hits;
    state.pre_admitted_orthant_hits = pre_admitted_orthant_hits;
    state.completed = completed;
    state.native_records = native_records;
    state.routed = routed;
    state.route_masks = route_masks;
    state.route_joint_support_masks_pruned = meta.route_joint_support_masks_pruned;
    state.initial_domain_count = initial_domain_count;
    state.initial_entry_domains_inspected = initial_entry_domains_inspected;
    let edge_count = edges.len();
    let closure_started = Instant::now();
    let mut closure = Tracker::from_parts(meta.closure, &flags, &edges)?;
    drop(edges);
    phases.since("closure_csr_build", closure_started);
    let started = Instant::now();
    closure.restore(state.queue.domains.len(), initial_domain_count)?;
    phases.since("closure_validate", started);
    let closure_seconds = closure_started.elapsed().as_secs_f64();
    state.closure = std::cell::RefCell::new(closure);
    state.parallel = meta.parallel;
    state.uncommitted = meta.uncommitted;
    let started = Instant::now();
    state.restore_checkpoint_progress(meta.progress)?;
    state.streams = meta.streams;
    state.validate_restored_streams()?;
    phases.since("progress_and_streams", started);
    let started = Instant::now();
    validate_ledger_closure(&state)?;
    phases.since("ledger_closure_cross_check", started);
    let report = json!({"verify_seconds":verify_seconds,"decode_seconds":decode_seconds,
        "validate_seconds":validate_started.elapsed().as_secs_f64(),"closure_seconds":closure_seconds,
        "phase_seconds":phases.json(),
        "domains":state.queue.domains.len(),"dependency_edges":edge_count,
        "records":state.records.borrow().total(),"records_accepted_events_derived":derived,
        "committed_domains":state.published_count(),
        "committed_events":state.events});
    Ok(Restored {
        state,
        inputs: meta.inputs,
        input_frontiers: meta.input_frontiers,
        report,
    })
}

/// Migration of a Ready checkpoint written before the sidecar kept the
/// accepted-events aggregate: stream every record segment once, in parallel,
/// retaining one line per segment reader. Every native record must carry its
/// count, and the native records must match the persisted native counter.
fn derive_accepted_events(sidecar: &Sidecar, native_records: usize) -> Result<usize, String> {
    #[derive(serde::Deserialize)]
    struct Probe<'a> {
        #[serde(borrow, default)]
        record_kind: Option<std::borrow::Cow<'a, str>>,
        #[serde(default)]
        accepted_events: Option<usize>,
    }
    let files = sidecar.files();
    #[cfg(not(target_arch = "wasm32"))]
    let parts = files.parts().par_iter();
    #[cfg(target_arch = "wasm32")]
    let parts = files.parts().iter();
    let contributions = parts.map(|part| {
        let (mut natives, mut accepted) = (0usize, 0usize);
        records::read_part(sidecar.directory(), part, &mut |line| {
            let probe: Probe<'_> = serde_json::from_slice(line)
                .map_err(|e| format!("invalid checkpoint record line: {e}"))?;
            if probe.record_kind.as_deref() != Some("delegated_not_inspected") {
                let events = probe
                    .accepted_events
                    .ok_or("ready checkpoint native record has no accepted-events count")?;
                natives += 1;
                accepted = accepted
                    .checked_add(events)
                    .ok_or("checkpoint accepted-events overflow")?;
            }
            Ok(())
        })?;
        Ok::<_, String>((natives, accepted))
    });
    #[cfg(not(target_arch = "wasm32"))]
    let (natives, accepted) = contributions.try_reduce(
        || (0, 0),
        |a, b| {
            Ok((
                a.0 + b.0,
                a.1.checked_add(b.1)
                    .ok_or("checkpoint accepted-events overflow")?,
            ))
        },
    )?;
    #[cfg(target_arch = "wasm32")]
    let (natives, accepted) = {
        let mut contributions = contributions;
        contributions.try_fold((0usize, 0usize), |a, b| {
            let b = b?;
            Ok::<_, String>((
                a.0 + b.0,
                a.1.checked_add(b.1)
                    .ok_or("checkpoint accepted-events overflow")?,
            ))
        })?
    };
    if natives != native_records {
        return Err("ready checkpoint records/publications disagree".into());
    }
    Ok(accepted)
}

fn g2_cell<const N: usize>(domain: &Domain<N>) -> Cell {
    Cell {
        owner: domain.owner.to_vec(),
        lower: domain.lower.clone(),
        upper: domain.upper.clone(),
        rank: domain.rank,
        powers: domain.powers,
    }
}

/// Exact cover of every G2' record: `Q <= residual u (lent scopes of its
/// anchors)`, decided by the lattice predicate (`Cell::covered_by_union`),
/// with lattice-point enumeration when the region budget cannot decide.
pub(super) fn validate_g2_cover<const N: usize>(
    log: &G2Log,
    domains: &[CompactDomain<N>],
) -> Result<(), String> {
    let mut by_id = std::collections::HashMap::with_capacity(log.rows.len());
    for row in &log.rows {
        by_id.insert(row.id, *row);
    }
    #[cfg(not(target_arch = "wasm32"))]
    let rows = log.rows.par_iter();
    #[cfg(target_arch = "wasm32")]
    let rows = log.rows.iter();
    let failures = rows
        .filter(|row| matches!(row.kind, kind::G2_RESIDUAL | kind::G2_FULL_COVER))
        .filter(|row| {
            let q = domains[row.id as usize].expand();
            let mut targets = Vec::with_capacity(row.anchors_len as usize + 1);
            if row.kind == kind::G2_RESIDUAL {
                let mut residual = q.clone();
                residual.powers =
                    super::super::g2::residual_powers(q.powers, row.band.0, row.band.1);
                targets.push(g2_cell(&residual));
            }
            for anchor in log.anchors_of(row) {
                let Some(anchor_row) = by_id.get(anchor) else {
                    return true;
                };
                let domain = domains[*anchor as usize].expand();
                targets.push(g2_cell(&lent_scope(&domain, anchor_row)));
            }
            let refs: Vec<&Cell> = targets.iter().collect();
            let whole = g2_cell(&q);
            let covered = whole
                .covered_by_union(&refs, 1 << 16)
                .or_else(|| whole.brute_force_covered_by_union(&refs, 1 << 20));
            covered != Some(true)
        })
        .count();
    if failures != 0 {
        return Err(format!(
            "checkpoint G2' record not covered by its residual and anchors ({failures} records)"
        ));
    }
    Ok(())
}

/// Checkpoint dependency endpoints are u32 (the edge section's width).
fn edge_key(source: usize, target: usize) -> Result<u64, String> {
    let endpoint = |id: usize| u32::try_from(id).map_err(|_| "dependency endpoint exceeds u32");
    Ok(u64::from(endpoint(source)?) << 32 | u64::from(endpoint(target)?))
}

/// Ledger/closure cross-check that replaces the former record scan. The
/// sidecar inventory (record and native counts) never depends on the
/// dependency monitor and is checked first; the per-ID seal and required
/// edge checks need an available monitor (an explicitly unavailable one is
/// not a completion claim).
pub(super) fn validate_ledger_closure<const N: usize>(state: &State<N>) -> Result<(), String> {
    let ledger = state.queue.delegation.as_ref();
    if state.records.borrow().total() != state.published_count()
        || state.native_records != ledger.map_or(state.queue.next, |l| l.native_publications())
    {
        return Err("checkpoint records/publications disagree".into());
    }
    let closure = state.closure.borrow();
    let total = state.queue.domains.len();
    if closure.json(total, state.initial_domain_count)["available"] != true {
        return Ok(());
    }
    let mut required = std::collections::HashSet::new();
    let mut unsealed_inspected = 0usize;
    for id in 0..total {
        let (expected, edges) = match ledger {
            Some(ledger) => ledger.closure_expectation(id),
            None => ((id < state.queue.next, false), [None, None]),
        };
        let actual = closure
            .local_status(id)
            .ok_or("dependency closure inventory mismatch")?;
        let matches = match ledger {
            Some(_) => actual == expected,
            // Without a ledger the seal depends on the record's frontiers:
            // only inspection is per ID, and the unsealed inspections are
            // bounded by the frontier count below. Unlike the former record
            // scan, this cannot detect a sealed native that kept frontiers.
            None => actual.0 == expected.0 && (expected.0 || !actual.1),
        };
        if !matches {
            return Err(if expected == (false, false) {
                "dependency sealed or inspected an unpublished node".into()
            } else {
                "dependency seal disagrees with native publication".into()
            });
        }
        unsealed_inspected += usize::from(actual.0 && !actual.1);
        for (index, target) in edges.into_iter().enumerate() {
            let Some(target) = target else {
                continue;
            };
            if index == 1 && target >= state.initial_domain_count {
                return Err("dependency partial anchor outside initial prefix".into());
            }
            if required.try_reserve(1).is_err() {
                return Err("dependency record validation allocation".into());
            }
            required.insert(edge_key(id, target)?);
        }
    }
    if ledger.is_none() && unsealed_inspected > state.frontiers {
        return Err("dependency seal disagrees with native publication".into());
    }
    // G2' anchor edges.
    if let Some(log) = ledger.and_then(|l| l.g2()) {
        for row in &log.rows {
            for &anchor in log.anchors_of(row) {
                if required.try_reserve(1).is_err() {
                    return Err("dependency record validation allocation".into());
                }
                required.insert(edge_key(row.id as usize, anchor as usize)?);
            }
        }
    }
    // Internal iteration over the CSR and the log, stopping once every
    // required edge was seen (order unspecified; the check does not need one).
    if !required.is_empty()
        && let ControlFlow::Break(Err(error)) =
            closure.try_for_each_edge(|source, target| match edge_key(source, target) {
                Err(error) => ControlFlow::Break(Err(error)),
                Ok(key) => {
                    required.remove(&key);
                    if required.is_empty() {
                        ControlFlow::Break(Ok(()))
                    } else {
                        ControlFlow::Continue(())
                    }
                }
            })
    {
        return Err(error);
    }
    if !required.is_empty() {
        return Err("dependency alias or partial-anchor edge missing".into());
    }
    Ok(())
}

/// The queue's own sections of a state checkpoint (meta queue counters,
/// domains, the raw index section), for offline index measurements that do
/// not need the ledger, closure or records.
#[cfg(test)]
pub(in super::super) struct QueueSections<const N: usize> {
    pub queue: super::super::queue::QueueMetadata,
    pub domains: Vec<super::super::queue::CompactDomain<N>>,
    index: Vec<u8>,
    identity: Identity,
}

#[cfg(test)]
impl<const N: usize> QueueSections<N> {
    /// A fresh decode of the index section.
    pub fn buckets(&self) -> Result<super::super::queue::StoredBuckets, String> {
        sections::read_index(&self.index, &self.identity)
    }

    /// Whether `queue` writes this index section byte for byte (CP5).
    pub fn index_round_trips(&self, queue: &Queue<N>) -> Result<bool, String> {
        let mut bytes = Vec::new();
        sections::write_index(&mut bytes, &self.identity, &queue.checkpoint_buckets())?;
        Ok(bytes == self.index)
    }

    /// The index payload decoded as another serde shape of the same bytes.
    pub fn decode_index_as<T: serde::de::DeserializeOwned>(&self) -> Result<T, String> {
        sections::decode_payload(&self.index[sections::HEADER_BYTES..])
    }
}

#[cfg(test)]
pub(in super::super) fn read_queue_sections<const N: usize>(
    dir: &Path,
) -> Result<QueueSections<N>, String> {
    let manifest = super::manifest::read(&dir.join("latest.json"))?;
    if !crate::application::routed_campaign::storage::compatible_width(manifest.arity as usize, N) {
        return Err("checkpoint coordinate arity".into());
    }
    let identity = Identity {
        arity: manifest.arity as usize,
        ready: manifest.publication_policy == "ready",
        semantics: manifest.walk_semantics_version,
    };
    let s = &manifest.sections;
    let plain = |section: Section| {
        s.plain(section)
            .ok_or_else(|| format!("state manifest is missing the {} section", section.name()))
    };
    let meta = sections::read_meta(&read_section(
        dir,
        &plain(Section::Meta)?.file,
        plain(Section::Meta)?.bytes,
    )?)?;
    let mut domains = Vec::new();
    for segment in &s
        .domains
        .as_ref()
        .ok_or("state manifest is missing the domains section")?
        .segments
    {
        let first = usize::try_from(segment.first).map_err(|_| "checkpoint segment range")?;
        let count = usize::try_from(segment.count).map_err(|_| "checkpoint segment range")?;
        sections::read_domains::<N>(
            &read_section(dir, &segment.file, segment.bytes)?,
            &identity,
            first,
            count,
            &mut domains,
        )?;
    }
    let index = plain(Section::Index)?;
    let index = read_section(dir, &index.file, index.bytes)?;
    Ok(QueueSections {
        queue: meta.queue,
        domains,
        index,
        identity,
    })
}
