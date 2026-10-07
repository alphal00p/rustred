//! Epoch-only typed binary record sidecars. Ready/CP5 retain their independent
//! JSON-line store. Sealed segments are append-only and authenticated on the
//! same stream consumed by readers; final JSON is an optional projection.
use super::super::checkpoint::{manifest::Segment, sections::HashingWriter};
use super::super::execution::records::Annotations;
use super::records::{typed::Record, wire};
use serde::ser::{Error as _, Serialize, SerializeMap, SerializeSeq, Serializer};
use serde_json::Value;
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Committed records reach the open segment in batches of whole frames of
/// about this size: one write(2) per batch, not per record, on the
/// coordinator's commit path. A batch not yet written stays readable in RAM.
const BATCH_BYTES: usize = 64 << 10;

pub(in crate::application::routed_campaign::walking) enum RecordSink {
    /// Non-checkpointed runs and unit tests.
    Memory(Vec<Record>),
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
    pub fn push(&mut self, record: Record) -> Result<(), String> {
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
    pub fn take_memory(&mut self) -> Vec<Record> {
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
            Self::Memory(records) => records
                .iter()
                .map(|r| r.project().expect("valid record JSON projection"))
                .collect(),
            Self::Sidecar(sidecar) => {
                let mut out = Vec::new();
                sidecar
                    .files()
                    .for_each_record(|record| {
                        out.push(record.project()?);
                        Ok(())
                    })
                    .expect("readable record sidecar");
                out
            }
        }
    }
}

/// Streaming writer over the checkpoint directory's record segments.
pub(in crate::application::routed_campaign::walking) struct Sidecar {
    directory: PathBuf,
    /// Sealed segments tiling `[0, sealed)` in generation order; the next
    /// manifest lists all of them.
    closed: Vec<Segment>,
    sealed: usize,
    /// Generation of the next segment, reserved by the store so that it never
    /// collides with an orphan; its file is created by the first push.
    generation: u64,
    open: Option<Open>,
    /// Serialized whole frames committed after the open segment's `written`
    /// ones, not yet written (at most about `BATCH_BYTES`), and their count.
    /// A failed write leaves them here: `files()` still reads them.
    batch: Vec<u8>,
    batched: usize,
    /// The first write or seal failure; the sidecar refuses further use.
    failed: Option<String>,
}

struct Open {
    file: String,
    /// Unbuffered: bytes reach the file only as whole-frame batches, so the
    /// file holds `written` whole frames (after a failed write, possibly
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
    pub fn push(&mut self, record: &Record) -> Result<(), String> {
        self.healthy()?;
        let start = self.batch.len();
        if let Err(e) = wire::append(record, &mut self.batch) {
            self.batch.truncate(start);
            return Err(format!("record sidecar serialization failed: {e}"));
        }
        if self.open.is_none() {
            let file = file_name(self.generation);
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
    /// One write of every batched line. On failure the frames stay batched
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
    /// frames, then a copy of the unwritten batch.
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
pub(in crate::application::routed_campaign::walking) struct Part {
    pub file: String,
    pub count: u64,
    pub sealed: Option<(u64, String)>,
}

/// Owned list of the record files of one sidecar, in publication order.
#[derive(Clone, Debug)]
pub(in crate::application::routed_campaign::walking) struct SidecarFiles {
    directory: PathBuf,
    parts: Vec<Part>,
    /// Whole frames committed after the parts, held in RAM.
    batch: Vec<u8>,
    batched: usize,
    total: usize,
}

impl SidecarFiles {
    /// The record files; `total` also counts the frames still in RAM.
    pub fn parts(&self) -> &[Part] {
        &self.parts
    }
    pub fn total(&self) -> usize {
        self.total
    }
    /// Decode each frame in publication order; sealed-file authentication and
    /// the semantic visitor consume the same opened reader.
    pub fn for_each_record(
        &self,
        mut visit: impl FnMut(Record) -> Result<(), String>,
    ) -> Result<(), String> {
        for part in &self.parts {
            read_part(&self.directory, part, &mut visit)?;
        }
        let mut input = self.batch.as_slice();
        for _ in 0..self.batched {
            visit(wire::read(&mut input).map_err(|e| format!("epoch record batch: {e}"))?)?;
        }
        if !input.is_empty() {
            return Err("epoch record batch trailing bytes".into());
        }
        Ok(())
    }
}

/// Fixed generation naming is part of the new binary inventory schema.
pub(in crate::application::routed_campaign::walking) fn file_name(generation: u64) -> String {
    format!("records-{generation:020}.bin")
}

/// Read and authenticate exactly the same opened bytes delivered to the
/// semantic callback. Unsealed failed tails expose only committed whole frames.
pub(in crate::application::routed_campaign::walking) fn read_part(
    directory: &Path,
    part: &Part,
    visit: &mut impl FnMut(Record) -> Result<(), String>,
) -> Result<(), String> {
    let path = directory.join(&part.file);
    let metadata = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", part.file))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(format!(
            "record segment {} must be a regular non-symlink file",
            part.file
        ));
    }
    let mut reader = BufReader::with_capacity(
        1 << 20,
        File::open(&path).map_err(|e| format!("{}: {e}", part.file))?,
    );
    let mut hashed = Hashed {
        input: &mut reader,
        hasher: blake3::Hasher::new(),
        bytes: 0,
    };
    for _ in 0..part.count {
        visit(wire::read(&mut hashed).map_err(|e| format!("{}: {e}", part.file))?)?;
    }
    if let Some((bytes, digest)) = &part.sealed {
        let mut tail = [0; 1];
        if hashed.read(&mut tail).map_err(|e| e.to_string())? != 0
            || hashed.bytes != *bytes
            || hashed.hasher.finalize().to_hex().as_str() != digest
        {
            return Err(format!(
                "checkpoint state checksum or length mismatch: {}",
                part.file
            ));
        }
    }
    Ok(())
}
struct Hashed<R> {
    input: R,
    hasher: blake3::Hasher,
    bytes: u64,
}
impl<R: Read> Read for Hashed<R> {
    fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
        let n = self.input.read(bytes)?;
        self.hasher.update(&bytes[..n]);
        self.bytes = self
            .bytes
            .checked_add(n as u64)
            .ok_or_else(|| std::io::Error::other("record byte count overflow"))?;
        Ok(n)
    }
}

/// A finished walk's records left in the checkpoint sidecar.
#[derive(Clone)]
pub(in crate::application::routed_campaign::walking) struct Streamed {
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
        self.files.for_each_record(|record| {
            let mut record = record.project()?;
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

/// Diagnostic-only bridge for existing independent JSON-row consumers. The
/// binary reader is consumed and authenticated in-place; only one projected
/// row is retained. Native publication and restore never use this adapter.
pub(in crate::application::routed_campaign::walking) struct JsonLines<R> {
    input: R,
    remaining: usize,
    line: std::io::Cursor<Vec<u8>>,
    ended: bool,
}
impl<R: Read> JsonLines<R> {
    pub fn new(input: R, remaining: usize) -> Self {
        Self {
            input,
            remaining,
            line: std::io::Cursor::new(Vec::new()),
            ended: false,
        }
    }
}
impl<R: Read> Read for JsonLines<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let n = self.line.read(output)?;
            if n != 0 {
                return Ok(n);
            }
            if self.ended {
                return Ok(0);
            }
            if self.remaining == 0 {
                if self.input.read(&mut [0])? != 0 {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "epoch record segment trailing bytes",
                    ));
                }
                self.ended = true;
                return Ok(0);
            }
            let value = wire::read(&mut self.input)?
                .project()
                .map_err(std::io::Error::other)?;
            let mut bytes = serde_json::to_vec(&value).map_err(std::io::Error::other)?;
            bytes.push(b'\n');
            self.line = std::io::Cursor::new(bytes);
            self.remaining -= 1;
        }
    }
}

impl Serialize for Domains<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut sequence = serializer.serialize_seq(Some(self.0.total()))?;
        let mut failure = None;
        let read = self.0.files.for_each_record(|record| {
            let mut record = record.project()?;
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
