//! Resume pipeline: every referenced file's length and digest is verified in
//! parallel before any section is decoded; then the same semantic checks as
//! the previous single-file codec run over the restored parts. Records are
//! never loaded: the record sidecar is reconstructed from the manifest's
//! segment list and cross-checked through the ledger and closure state.
use super::super::{
    descendant_closure::Tracker,
    execution::{
        State,
        records::{self, RecordSink, Sidecar},
    },
    queue::Queue,
};
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
    let derived = ready && meta.records_accepted_events.is_none();
    let records_accepted_events = match meta.records_accepted_events {
        Some(total) if ready => total,
        Some(0) | None if !ready => 0,
        Some(_) => return Err("ordered checkpoint carries ready accepted-event accounting".into()),
        // Written before the sidecar kept the aggregate: derive it once.
        None => derive_accepted_events(&sidecar, native_records)?,
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
    let mut closure = Tracker::from_parts(meta.closure, &flags, edges)?;
    closure.restore(state.queue.domains.len(), initial_domain_count)?;
    state.closure = std::cell::RefCell::new(closure);
    state.parallel = meta.parallel;
    state.uncommitted = meta.uncommitted;
    state.restore_checkpoint_progress(meta.progress)?;
    state.streams = meta.streams;
    state.validate_restored_streams()?;
    validate_ledger_closure(&state)?;
    let report = json!({"verify_seconds":verify_seconds,"decode_seconds":decode_seconds,
        "validate_seconds":validate_started.elapsed().as_secs_f64(),
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
    let (natives, accepted) = files
        .parts()
        .par_iter()
        .map(|part| {
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
        })
        .try_reduce(
            || (0, 0),
            |a, b| {
                Ok((
                    a.0 + b.0,
                    a.1.checked_add(b.1)
                        .ok_or("checkpoint accepted-events overflow")?,
                ))
            },
        )?;
    if natives != native_records {
        return Err("ready checkpoint records/publications disagree".into());
    }
    Ok(accepted)
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
            // only inspection is per ID, unsealed inspections are aggregated.
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
    for (source, target) in closure.dependencies() {
        if required.is_empty() {
            break;
        }
        required.remove(&edge_key(source, target)?);
    }
    if !required.is_empty() {
        return Err("dependency alias or partial-anchor edge missing".into());
    }
    Ok(())
}
