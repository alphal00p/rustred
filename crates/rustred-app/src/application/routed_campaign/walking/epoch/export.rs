//! The S2 final export: a non-resumable, digest-bound image of the final
//! epoch state for the offline oracles (walk-verify-closure's epoch reader
//! and the audit). This historical directory was written once at finalization.
//! Historical S2 directories remain an offline format, never imported by CP6.
//! New checkpoint-enabled public runs use the S3 publisher and validated resume.
//!
//! Files (little-endian; each binary file starts with an 8-byte magic, a
//! u32 version, a u32 arity and a u64 count):
//! - `domains-1.seg`: count x canonical image (37 + 4N bytes, `job.rs`);
//! - `nodes-1.bin`: count x u8 flags (bit0 sealed, bit1 inspected, bit2
//!   closed by the final forced refresh, bit3 anchored, bit4 residual);
//! - `ledger6-1.bin`: count x u64 ledger6 words (§4.1);
//! - `edges-1.seg`: count = runs; runs `(source u32, n u32, targets u32 x n)`;
//! - `anchors-1.bin`: the anchor layout v2 (§11.7 as amended, note D19);
//! - `records-<generation>.bin`: typed binary records in merge order;
//! - `epoch-export.json`: the manifest (blake3 and length of every file).
use super::job::{Writer, write_image};
use super::state::EpochState;
use serde_json::{Value, json};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub(super) const MANIFEST: &str = "epoch-export.json";
pub(super) const FORMAT: &str = "RUSTRED-EPOCH-EXPORT";
pub(super) const GENERATION: u64 = 1;

pub(super) const DOMAINS_MAGIC: &[u8; 8] = b"EPDOMS01";
pub(super) const NODES_MAGIC: &[u8; 8] = b"EPNODE01";
pub(super) const LEDGER_MAGIC: &[u8; 8] = b"EPLED601";
pub(super) const EDGES_MAGIC: &[u8; 8] = b"EPEDGE01";

/// Refuse to reuse a directory that already holds an export or a CP5 state.
pub(super) fn prepare_directory(directory: &Path) -> Result<(), String> {
    fs::create_dir_all(directory).map_err(|e| {
        format!(
            "cannot create epoch export directory {}: {e}",
            directory.display()
        )
    })?;
    let mut entries =
        fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    if entries.next().is_some() {
        return Err(format!(
            "epoch export directory {} is not empty (S2 writes one final export; --resume is S3)",
            directory.display()
        ));
    }
    Ok(())
}

fn header(magic: &[u8; 8], arity: usize, count: u64) -> Vec<u8> {
    let mut out = magic.to_vec();
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(arity as u32).to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out
}

/// Write `bytes` to `directory/name` (fsync) and return (length, blake3).
fn write_file(directory: &Path, name: &str, bytes: &[u8]) -> Result<Value, String> {
    let path = directory.join(name);
    let mut file = File::create(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(json!({"bytes":bytes.len(),"blake3":blake3::hash(bytes).to_hex().to_string()}))
}

pub(super) fn file_blake3(path: &Path) -> Result<(u64, String), String> {
    let mut file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut bytes = 0u64;
    loop {
        let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
        bytes += n as u64;
    }
    Ok((bytes, hasher.finalize().to_hex().to_string()))
}

pub(super) struct ExportParts<'a> {
    pub binding: String,
    pub owners: &'a [String],
    pub metadata: Value,
    pub counters: Value,
    pub inputs: &'a [Value],
    pub input_frontiers: &'a [Value],
    pub closure: Value,
    pub records: Value,
    pub extra: Value,
}

/// Write every section, then the manifest last (atomic rename).
pub(super) fn write<const N: usize>(
    directory: &Path,
    state: &EpochState<N>,
    parts: ExportParts<'_>,
) -> Result<PathBuf, String> {
    if state.poisoned {
        return Err("refusing to export a poisoned epoch state (P3 did not complete)".into());
    }
    let count = state.store.len() as u64;
    let mut files = serde_json::Map::new();
    let mut domains = Writer(header(DOMAINS_MAGIC, N, count));
    for image in &state.store.domains {
        write_image(&mut domains, image);
    }
    files.insert(
        "domains".into(),
        json!({"file":"domains-1.seg","meta":write_file(directory, "domains-1.seg", &domains.0)?}),
    );
    let mut nodes = header(NODES_MAGIC, N, count);
    for (id, &flags) in state.nodes.iter().enumerate() {
        let closed = if state.tracker.closed(id) == Some(true) {
            4
        } else {
            0
        };
        nodes.push((flags & !4) | closed);
    }
    files.insert(
        "nodes".into(),
        json!({"file":"nodes-1.bin","meta":write_file(directory, "nodes-1.bin", &nodes)?}),
    );
    let mut ledger = header(LEDGER_MAGIC, N, count);
    for word in state.ledger.words() {
        ledger.extend_from_slice(&word.to_le_bytes());
    }
    files.insert(
        "ledger6".into(),
        json!({"file":"ledger6-1.bin","meta":write_file(directory, "ledger6-1.bin", &ledger)?}),
    );
    let mut edges = header(EDGES_MAGIC, N, state.edges.runs());
    for word in state.edges.log() {
        edges.extend_from_slice(&word.to_le_bytes());
    }
    files.insert(
        "edges".into(),
        json!({"file":"edges-1.seg","meta":write_file(directory, "edges-1.seg", &edges)?}),
    );
    files.insert(
        "anchors".into(),
        json!({"file":"anchors-1.bin",
            "meta":write_file(directory, "anchors-1.bin", &state.anchors.encode()?)?}),
    );
    let manifest = json!({"format":FORMAT,"schema":2,"stage":"S5","kind":"final_state_export",
        "resumable":false,"generation":GENERATION,"publication_policy":"epoch",
        "record_schema":super::records::wire::RECORD_SCHEMA,
        "walk_semantics_version":super::EPOCH_WALK_SEMANTICS_VERSION,"arity":N,
        "request":parts.binding,"owners":parts.owners,
        "executable":std::env::current_exe().ok().and_then(|p| file_blake3(&p).ok()).map(|d| d.1),
        "files":files,"records":parts.records,"metadata":parts.metadata,
        "counters":parts.counters,"inputs":parts.inputs,"input_frontiers":parts.input_frontiers,
        "closure":parts.closure,"k":state.k,"watermark":count,"p0":state.p0,
        "ledger6_counts":state.ledger.counts().json(),
        "records_digest":state.edges.records_digest(),"edge_digest":state.edges.edge_digest(),
        "edge_runs":state.edges.runs(),"edges":state.edges.edges(),
        "self_edges":state.edges.self_edges(),"extra":parts.extra,
        "family_closure_claim":false});
    let text = serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?;
    let staging = directory.join(format!("{MANIFEST}.tmp"));
    let mut file = File::create(&staging).map_err(|e| format!("{}: {e}", staging.display()))?;
    file.write_all(&text)
        .and_then(|()| file.sync_all())
        .map_err(|e| format!("{}: {e}", staging.display()))?;
    let path = directory.join(MANIFEST);
    fs::rename(&staging, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    File::open(directory)
        .and_then(|d| d.sync_all())
        .map_err(|e| format!("{}: {e}", directory.display()))?;
    Ok(path)
}

/// Sticky poison file of an engine-fatal (C5) merge (§6.6; no generation).
pub(super) fn write_poison(directory: &Path, k: u64, reason: &str) -> Result<(), String> {
    let text = serde_json::to_vec_pretty(&json!({"k":k,"reason":reason,
        "binary":std::env::current_exe().ok().and_then(|p| file_blake3(&p).ok()).map(|d| d.1),
        "engine_certification_void":true}))
    .map_err(|e| e.to_string())?;
    fs::write(directory.join(format!("poison-{k}.json")), text).map_err(|e| e.to_string())
}
