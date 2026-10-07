//! Committed domain records. Non-checkpointed runs keep them in RAM; a
//! checkpointed run serializes each record at commit time and appends it, in
//! batches of whole lines, to the checkpoint's append-only
//! `records-<G>.jsonl` segments (one JSON value per line, the CP5 records
//! section), retaining only aggregates and at most one unwritten batch.
//!
//! Segment `G` holds the records committed after the previous save and
//! before the save of generation `G`; the store seals it (fsync, digest) and
//! lists every sealed segment in the manifest. An unsealed segment is
//! referenced by no manifest: a crash or failed save leaves an orphan that
//! resume ignores and segment-aware cleanup removes. `result.json` reads the
//! records back once, in publication order, applying the finalization
//! annotations from the ledger and the dependency monitor.
use super::super::checkpoint::{
    manifest::{Section, Segment},
    sections::HashingWriter,
};
use super::super::delegation::Resolution;
use serde::ser::{Error as _, Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::{Value, json};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// One record line may not exceed this; a longer line is corruption.
const MAX_RECORD_LINE_BYTES: u64 = 1 << 30;
/// Committed records reach the open segment in batches of whole lines of
/// about this size: one write(2) per batch, not per record, on the
/// coordinator's commit path. A batch not yet written stays readable in RAM.
const BATCH_BYTES: usize = 64 << 10;

pub(in super::super) enum RecordSink {
    /// Non-checkpointed runs and unit tests.
    Memory(Vec<Value>),
    /// Checkpointed runs: no per-record state stays in RAM.
    Sidecar(Sidecar),
}

impl RecordSink {
    /// Checked before a publication that cannot be undone. Memory reserves
    /// the slot, so the push that follows cannot fail; a sidecar only checks
    /// its health, and its write can still fail after the publication: the
    /// caller then fails the run, and a failed prefix is never checkpointed.
    pub fn reserve_one(&mut self) -> Result<(), String> {
        match self {
            Self::Memory(records) => records
                .try_reserve(1)
                .map_err(|_| "record allocation".to_owned()),
            Self::Sidecar(sidecar) => sidecar.healthy(),
        }
    }
    /// Append one committed record. A failure is the publisher's error; no
    /// placeholder record is ever written in its place.
    pub fn push(&mut self, record: Value) -> Result<(), String> {
        match self {
            Self::Memory(records) => {
                records
                    .try_reserve(1)
                    .map_err(|_| "record allocation".to_owned())?;
                records.push(record);
                Ok(())
            }
            Self::Sidecar(sidecar) => sidecar.push(&record),
        }
    }
    pub fn total(&self) -> usize {
        match self {
            Self::Memory(records) => records.len(),
            Self::Sidecar(sidecar) => sidecar.total(),
        }
    }
    /// Owner-batched and other in-memory reports move the rows out.
    pub fn take_memory(&mut self) -> Vec<Value> {
        match self {
            Self::Memory(records) => std::mem::take(records),
            Self::Sidecar(_) => Vec::new(),
        }
    }
    /// Test-facing read-back in publication order (Memory: clone; Sidecar:
    /// every sealed segment, then the open tail, read from disk).
    #[cfg(test)]
    pub fn snapshot(&self) -> Vec<Value> {
        match self {
            Self::Memory(records) => records.clone(),
            Self::Sidecar(sidecar) => {
                let mut out = Vec::new();
                sidecar
                    .files()
                    .for_each_record(|record| {
                        out.push(record);
                        Ok(())
                    })
                    .expect("readable record sidecar");
                out
            }
        }
    }
}

/// Streaming writer over the checkpoint directory's record segments.
pub(in super::super) struct Sidecar {
    directory: PathBuf,
    /// Sealed segments tiling `[0, sealed)` in generation order; the next
    /// manifest lists all of them.
    closed: Vec<Segment>,
    sealed: usize,
    /// Generation of the next segment, reserved by the store so that it never
    /// collides with an orphan; its file is created by the first push.
    generation: u64,
    open: Option<Open>,
    /// Serialized whole lines committed after the open segment's `written`
    /// ones, not yet written (at most about `BATCH_BYTES`), and their count.
    /// A failed write leaves them here: `files()` still reads them.
    batch: Vec<u8>,
    batched: usize,
    /// The first write or seal failure; the sidecar refuses further use.
    failed: Option<String>,
}

struct Open {
    file: String,
    /// Unbuffered: bytes reach the file only as whole-line batches, so the
    /// file holds `written` whole lines (after a failed write, possibly
    /// followed by a partial batch that no reader consumes).
    writer: HashingWriter<File>,
    written: usize,
}

impl Open {
    /// fsync, digest of the bytes as written, then the directory entry: the
    /// segment is durable before any manifest can reference it. Borrowed, so
    /// that a failed seal leaves the tail readable (see `Sidecar::seal`).
    fn seal(&self, directory: &Path, generation: u64, first: usize) -> Result<Segment, String> {
        self.writer
            .get_ref()
            .sync_all()
            .map_err(|e| format!("cannot sync record segment {}: {e}", self.file))?;
        let (bytes, blake3) = self.writer.digest();
        File::open(directory)
            .and_then(|directory| directory.sync_all())
            .map_err(|e| format!("cannot sync checkpoint directory: {e}"))?;
        Ok(Segment {
            generation,
            file: self.file.clone(),
            first: first as u64,
            count: self.written as u64,
            bytes,
            blake3,
        })
    }
}

impl Sidecar {
    pub fn new(directory: PathBuf, generation: u64) -> Self {
        Self::restored(directory, Vec::new(), generation)
    }
    /// Reconstructed from the manifest's segment list; no record is read.
    pub fn restored(directory: PathBuf, closed: Vec<Segment>, generation: u64) -> Self {
        let sealed = closed.iter().map(|s| s.count as usize).sum();
        Self {
            directory,
            closed,
            sealed,
            generation,
            open: None,
            batch: Vec::new(),
            batched: 0,
            failed: None,
        }
    }
    pub fn directory(&self) -> &Path {
        &self.directory
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn closed(&self) -> &[Segment] {
        &self.closed
    }
    pub fn sealed_total(&self) -> usize {
        self.sealed
    }
    pub fn total(&self) -> usize {
        self.sealed + self.open.as_ref().map_or(0, |open| open.written) + self.batched
    }
    fn healthy(&self) -> Result<(), String> {
        self.failed.clone().map_or(Ok(()), Err)
    }
    fn fail(&mut self, error: String) -> String {
        self.failed.get_or_insert_with(|| error.clone());
        error
    }
    pub fn push(&mut self, record: &Value) -> Result<(), String> {
        self.healthy()?;
        let start = self.batch.len();
        if let Err(e) = serde_json::to_writer(&mut self.batch, record) {
            self.batch.truncate(start);
            return Err(format!("record sidecar serialization failed: {e}"));
        }
        self.batch.push(b'\n');
        if self.open.is_none() {
            let file = Section::Records.file_name(self.generation);
            let created = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.directory.join(&file))
                .map_err(|e| format!("cannot create record sidecar segment {file}: {e}"));
            match created {
                Ok(handle) => {
                    self.open = Some(Open {
                        file,
                        writer: HashingWriter::new(handle),
                        written: 0,
                    })
                }
                Err(e) => {
                    self.batch.truncate(start);
                    return Err(self.fail(e));
                }
            }
        }
        self.batched += 1;
        if self.batch.len() >= BATCH_BYTES {
            self.write_batch()?;
        }
        Ok(())
    }
    /// One write of every batched line. On failure the lines stay batched
    /// (read by `files()` for the failed run's report) and the sidecar
    /// refuses further use.
    fn write_batch(&mut self) -> Result<(), String> {
        let Some(open) = self.open.as_mut().filter(|_| self.batched > 0) else {
            return Ok(());
        };
        if let Err(e) = open.writer.write_all(&self.batch) {
            return Err(self.fail(format!("record sidecar write failed: {e}")));
        }
        open.written += self.batched;
        self.batched = 0;
        self.batch.clear();
        if self.batch.capacity() > 2 * BATCH_BYTES {
            self.batch = Vec::new(); // Do not pin one exceptional record's buffer.
        }
        Ok(())
    }
    /// Seal the open segment as part of the save of `generation` (fsync,
    /// digest from the bytes as written) and reserve `next` for the one after.
    /// Returns the new segment, or None when nothing was committed since the
    /// previous save.
    pub fn seal(&mut self, generation: u64, next: u64) -> Result<Option<Segment>, String> {
        self.healthy()?;
        if generation != self.generation || next <= generation {
            return Err("record sidecar generation disagrees with the checkpoint save".into());
        }
        self.write_batch()?;
        let segment = match &self.open {
            None => None,
            Some(open) => match open.seal(&self.directory, generation, self.sealed) {
                Ok(segment) => Some(segment),
                // The tail stays open: `files()` still reads every committed
                // record for the failed run's report, and the failed flag
                // keeps it out of every manifest.
                Err(e) => return Err(self.fail(e)),
            },
        };
        if let Some(segment) = &segment {
            self.open = None;
            self.sealed += segment.count as usize;
            self.closed.push(segment.clone());
        }
        self.generation = next;
        Ok(segment)
    }
    /// Read-only view of every committed record: sealed segments (length
    /// and digest re-verified while reading), the unsealed tail's written
    /// lines, then a copy of the unwritten batch.
    pub fn files(&self) -> SidecarFiles {
        let mut parts: Vec<Part> = self
            .closed
            .iter()
            .map(|segment| Part {
                file: segment.file.clone(),
                count: segment.count,
                sealed: Some((segment.bytes, segment.blake3.clone())),
            })
            .collect();
        if let Some(open) = &self.open {
            parts.push(Part {
                file: open.file.clone(),
                count: open.written as u64,
                sealed: None,
            });
        }
        SidecarFiles {
            directory: self.directory.clone(),
            parts,
            batch: self.batch.clone(),
            batched: self.batched,
            total: self.total(),
        }
    }
}

/// One record file: its record count and, when sealed, its manifest length
/// and blake3 digest.
#[derive(Clone, Debug)]
pub(in super::super) struct Part {
    pub file: String,
    pub count: u64,
    pub sealed: Option<(u64, String)>,
}

/// Owned list of the record files of one sidecar, in publication order.
#[derive(Clone, Debug)]
pub(in super::super) struct SidecarFiles {
    directory: PathBuf,
    parts: Vec<Part>,
    /// Whole lines committed after the parts, held in RAM.
    batch: Vec<u8>,
    batched: usize,
    total: usize,
}

impl SidecarFiles {
    /// The record files; `total` also counts the lines still in RAM.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
    pub fn total(&self) -> usize {
        self.total
    }
    /// Every record line (without its newline), in publication order.
    pub fn for_each_line(
        &self,
        mut visit: impl FnMut(&[u8]) -> Result<(), String>,
    ) -> Result<(), String> {
        for part in &self.parts {
            read_part(&self.directory, part, &mut visit)?;
        }
        // Serialized JSON holds no raw newline.
        for line in self.batch.split(|&b| b == b'\n').take(self.batched) {
            visit(line)?;
        }
        Ok(())
    }
    pub fn for_each_record(
        &self,
        mut visit: impl FnMut(Value) -> Result<(), String>,
    ) -> Result<(), String> {
        self.for_each_line(|line| {
            visit(
                serde_json::from_slice(line)
                    .map_err(|e| format!("invalid record sidecar line: {e}"))?,
            )
        })
    }
}

/// Stream one record file line by line, retaining one line at a time. A
/// sealed part must hold exactly `count` lines with the manifest's length
/// and digest; the unsealed tail yields its first `count` lines (a failed
/// write may have left a partial line after them).
pub(in super::super) fn read_part(
    directory: &Path,
    part: &Part,
    visit: &mut impl FnMut(&[u8]) -> Result<(), String>,
) -> Result<(), String> {
    let path = directory.join(&part.file);
    if fs::symlink_metadata(&path)
        .map_err(|e| format!("{}: {e}", part.file))?
        .file_type()
        .is_symlink()
    {
        return Err(format!("record segment {} may not be a symlink", part.file));
    }
    let mut reader = BufReader::with_capacity(
        1 << 20,
        File::open(&path).map_err(|e| format!("{}: {e}", part.file))?,
    );
    let mut hasher = blake3::Hasher::new();
    let mut bytes = 0u64;
    let mut line = Vec::new();
    for _ in 0..part.count {
        line.clear();
        let read = (&mut reader)
            .take(MAX_RECORD_LINE_BYTES + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| format!("{}: {e}", part.file))?;
        if read == 0 || line.last() != Some(&b'\n') {
            return Err(format!(
                "record segment {} is shorter than its record count",
                part.file
            ));
        }
        hasher.update(&line);
        bytes += read as u64;
        visit(&line[..line.len() - 1])?;
    }
    if let Some((expected_bytes, expected_digest)) = &part.sealed {
        let trailing = reader
            .fill_buf()
            .map_err(|e| format!("{}: {e}", part.file))?;
        if !trailing.is_empty()
            || bytes != *expected_bytes
            || hasher.finalize().to_hex().as_str() != expected_digest
        {
            return Err(format!(
                "checkpoint state checksum or length mismatch: {}",
                part.file
            ));
        }
    }
    Ok(())
}

/// Finalization annotations, applied to every record in the streaming
/// post-pass exactly as the former in-place mutation did: ledger
/// resolutions on delegated and partial records, and the dependency
/// monitor's closed flag on every record.
#[derive(Clone, Default)]
pub(in super::super) struct Annotations {
    resolutions: Option<Vec<Resolution>>,
    closed: Vec<Option<bool>>,
}

impl Annotations {
    pub fn new(
        resolutions: Option<Vec<Resolution>>,
        closure: &super::super::descendant_closure::Tracker,
    ) -> Self {
        #[cfg(test)]
        super::super::epoch::assert_large_finalization_allowed();
        Self {
            resolutions,
            closed: (0..closure.node_count())
                .map(|id| closure.closed(id))
                .collect(),
        }
    }
    pub fn apply(&self, record: &mut Value) {
        if let Some(by_id) = &self.resolutions {
            super::delegation::annotate_resolution(record, by_id);
        }
        if let Some(id) = record["id"]
            .as_u64()
            .and_then(|id| usize::try_from(id).ok())
        {
            record["descendant_closed"] = json!(self.closed.get(id).copied().flatten());
        }
    }
}

/// A finished walk's records left in the checkpoint sidecar.
#[derive(Clone)]
pub(in super::super) struct Streamed {
    files: SidecarFiles,
    annotations: Annotations,
    projection: Option<usize>,
}

impl Streamed {
    pub fn project_coordinates(&mut self, physical_arity: usize) {
        self.projection = Some(physical_arity);
    }
    pub fn new(sidecar: Sidecar, annotations: Annotations) -> Self {
        Self {
            projection: None,
            files: sidecar.files(),
            annotations,
        }
    }
    pub fn total(&self) -> usize {
        self.files.total()
    }
    pub fn segments(&self) -> usize {
        self.files.parts().len()
    }
    /// Materialize every annotated record (small runs and tests).
    pub fn collect(&self) -> Result<Vec<Value>, String> {
        let mut out = Vec::new();
        out.try_reserve_exact(self.total())
            .map_err(|_| "record allocation")?;
        self.files.for_each_record(|mut record| {
            self.annotations.apply(&mut record);
            if let Some(n) = self.projection {
                crate::application::routed_campaign::storage::project(&mut record, n);
            }
            out.push(record);
            Ok(())
        })?;
        Ok(out)
    }
    /// Pretty JSON of `document` with its `domains` entry streamed one record
    /// at a time; byte-identical to `serde_json::to_writer_pretty` of the
    /// materialized document, with one record resident at a time.
    pub fn write_json(&self, document: &Value, out: impl Write) -> Result<(), String> {
        let map = document
            .as_object()
            .ok_or("walk result document is not a JSON object")?;
        let mut serializer = serde_json::Serializer::pretty(out);
        let mut entries = serializer
            .serialize_map(Some(map.len()))
            .map_err(|e| e.to_string())?;
        for (key, value) in map {
            let entry = if key == "domains" {
                entries.serialize_entry(key, &Domains(self))
            } else {
                entries.serialize_entry(key, value)
            };
            entry.map_err(|e| e.to_string())?;
        }
        SerializeMap::end(entries).map_err(|e| e.to_string())
    }
}

struct Domains<'a>(&'a Streamed);

impl Serialize for Domains<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.total()))?;
        let mut failure = None;
        let read = self.0.files.for_each_record(|mut record| {
            self.0.annotations.apply(&mut record);
            if let Some(n) = self.0.projection {
                crate::application::routed_campaign::storage::project(&mut record, n);
            }
            sequence.serialize_element(&record).map_err(|e| {
                let message = e.to_string();
                failure = Some(e);
                message
            })
        });
        if let Some(error) = failure {
            return Err(error);
        }
        read.map_err(S::Error::custom)?;
        SerializeSeq::end(sequence)
    }
}

#[cfg(test)]
mod tests;
