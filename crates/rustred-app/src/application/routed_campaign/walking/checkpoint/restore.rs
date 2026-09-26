//! Resume pipeline: every referenced file's length and digest is verified in
//! parallel before any section is decoded; then the same semantic checks as
//! the previous single-file codec run over the restored parts.
use super::super::{descendant_closure::Tracker, execution::State, queue::Queue};
use super::manifest::{Manifest, Section};
use super::sections::{self, Identity};
use rayon::prelude::*;
use serde_json::{Value, json};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::Instant;

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
    manifest.files().par_iter().try_for_each(|f| {
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

fn read_section(dir: &Path, file: &str, bytes: u64) -> Result<Vec<u8>, String> {
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
    if manifest.arity as usize != N {
        return Err("checkpoint coordinate arity".into());
    }
    let ready = manifest.publication_policy == "ready";
    let identity = Identity {
        arity: N,
        ready,
        semantics: manifest.walk_semantics_version,
    };
    let decode_started = Instant::now();
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
    let nodes = plain(Section::Nodes)?;
    let flags = sections::read_nodes(&read_section(dir, &nodes.file, nodes.bytes)?, &identity)?;
    let mut edges = Vec::new();
    for segment in &s
        .edges
        .as_ref()
        .ok_or("state manifest is missing the edges section")?
        .segments
    {
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
    let ledger = s
        .ledger
        .as_ref()
        .map(|l| sections::read_ledger(&read_section(dir, &l.file, l.bytes)?, &identity))
        .transpose()?;
    let index = plain(Section::Index)?;
    let buckets = sections::read_index(&read_section(dir, &index.file, index.bytes)?, &identity)?;
    let mut records = Vec::new();
    for segment in &s
        .records
        .as_ref()
        .ok_or("state manifest is missing the records section")?
        .segments
    {
        let count = usize::try_from(segment.count).map_err(|_| "checkpoint segment range")?;
        sections::read_records(
            &read_section(dir, &segment.file, segment.bytes)?,
            count,
            &mut records,
        )?;
    }
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
    let queue = Queue::<N>::restore_from_parts(meta.queue, domains, buckets, ledger)?;
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
    let mut state = State::new(queue, frontiers, None);
    state.records = records;
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
    let mut closure = Tracker::from_parts(meta.closure, &flags, edges)?;
    closure.restore(state.queue.domains.len(), initial_domain_count)?;
    state.closure = std::cell::RefCell::new(closure);
    state.parallel = meta.parallel;
    state.uncommitted = meta.uncommitted;
    state.restore_checkpoint_progress(meta.progress)?;
    state.streams = meta.streams;
    state.validate_restored_streams()?;
    validate_closure_records(&state)?;
    let report = json!({"verify_seconds":verify_seconds,"decode_seconds":decode_seconds,
        "validate_seconds":validate_started.elapsed().as_secs_f64(),
        "domains":state.queue.domains.len(),"dependency_edges":edge_count,
        "records":state.records.len(),"committed_domains":state.published_count(),
        "committed_events":state.events});
    Ok(Restored {
        state,
        inputs: meta.inputs,
        input_frontiers: meta.input_frontiers,
        report,
    })
}

/// Record-based cross-check of the dependency seals against the published
/// records. Wave 2 replaces the record scan with a ledger/closure check.
pub(super) fn validate_closure_records<const N: usize>(state: &State<N>) -> Result<(), String> {
    let closure = state.closure.borrow();
    if closure.json(state.queue.domains.len(), state.initial_domain_count)["available"] != true {
        return Ok(()); // Explicitly unavailable monitoring is not a completion claim.
    }
    let mut recorded = Vec::new();
    recorded
        .try_reserve_exact(state.queue.domains.len())
        .map_err(|_| "dependency record validation allocation")?;
    recorded.resize(state.queue.domains.len(), false);
    let mut required = std::collections::HashSet::new();
    required
        .try_reserve(state.records.len())
        .map_err(|_| "dependency record validation allocation")?;
    for record in &state.records {
        let id = record["id"]
            .as_u64()
            .and_then(|id| usize::try_from(id).ok())
            .ok_or("dependency record ID missing")?;
        if id >= recorded.len()
            || recorded[id]
            || !state
                .queue
                .delegation
                .as_ref()
                .map_or(id < state.queue.next, |ledger| ledger.is_published(id))
        {
            return Err("dependency record is duplicate or unpublished".into());
        }
        recorded[id] = true;
        let status = if record["record_kind"] == "delegated_not_inspected" {
            let target = record["representative_id"]
                .as_u64()
                .and_then(|id| usize::try_from(id).ok())
                .ok_or("dependency alias representative missing")?;
            if state
                .queue
                .delegation
                .as_ref()
                .and_then(|l| l.delegated_to(id))
                != Some(target)
            {
                return Err("dependency alias does not match ledger".into());
            }
            required.insert((id, target));
            (false, true)
        } else {
            let inspected = record.get("error").is_some_and(Value::is_null)
                && (record["local_inspection_finished"] == true
                    || record["residual_inspection_finished"] == true);
            let frontiers = record["frontiers"]
                .as_array()
                .ok_or("dependency native frontier status missing")?;
            if record["record_kind"] == "partial_initial_overlap_inspection" {
                let target = record["initial_overlap"]["anchor_id"]
                    .as_u64()
                    .and_then(|id| usize::try_from(id).ok())
                    .ok_or("dependency partial anchor missing")?;
                if target >= state.initial_domain_count {
                    return Err("dependency partial anchor outside initial prefix".into());
                }
                required.insert((id, target));
            }
            (inspected, inspected && frontiers.is_empty())
        };
        if closure.local_status(id) != Some(status) {
            return Err("dependency seal disagrees with native publication".into());
        }
    }
    for (id, seen) in recorded.iter().enumerate() {
        if !seen && closure.local_status(id) != Some((false, false)) {
            return Err("dependency sealed or inspected an unpublished node".into());
        }
    }
    for edge in closure.dependencies() {
        required.remove(&edge);
    }
    if !required.is_empty() {
        return Err("dependency alias or partial-anchor edge missing".into());
    }
    Ok(())
}
