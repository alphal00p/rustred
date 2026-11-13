//! Append-only record streams with running digests, table sealing and the sealed-table verifier
//! (child-job.md 3.4.4, 3.9; ruling R-b: records are accepted ONLY from a sealed table).
//!
//! A table is SEALED iff OUTCOME says `sealed: true` with `bytes` and `blake3`, the file has exactly `bytes` bytes
//! with that BLAKE3, it ends with "\n", and its last line is an end record (`type` `end` or `partial`, matching
//! OUTCOME's `end`); when OUTCOME gives `records` the line count must match too. The verifier streams the file in
//! fixed chunks and keeps at most [`END_RECORD_MAX`] bytes of the last line: constant memory whatever the line
//! lengths.

use crate::canon::{self, CanonError, CanonicalDigest};
use crate::outcome::{EndKind, TableOutput};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::Path;

/// An end record longer than this cannot be one (the verifier's bound on the last line it keeps).
pub const END_RECORD_MAX: usize = 1 << 20;

/// The seal of a finished table (what goes into `OUTCOME.job.outputs.table`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TableSeal {
    pub bytes: u64,
    pub blake3: String,
    pub records: u64,
    pub rules: u64,
    pub end: EndKind,
    pub canonical_blake3: String,
}

impl TableSeal {
    /// Fill the seal fields of an OUTCOME table entry (sealed = true).
    pub fn apply(&self, t: &mut TableOutput) {
        t.bytes = Some(self.bytes);
        t.blake3 = Some(self.blake3.clone());
        t.records = Some(self.records);
        t.rules = Some(self.rules);
        t.end = Some(self.end);
        t.canonical_blake3 = Some(self.canonical_blake3.clone());
        t.sealed = true;
    }
}

/// State at a commit point (3.4.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPoint {
    pub bytes: u64,
    pub prefix_blake3: String,
    pub records: u64,
    pub rules: u64,
}

/// Position of one appended record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordPos {
    pub offset: u64,
    /// Line length including its "\n".
    pub bytes: u64,
    pub canon_blake3: String,
}

/// Error of the record writer.
#[derive(Debug)]
pub enum WriteError {
    Io(io::Error),
    /// The record has no canonical form (invalid JSON, duplicate keys...).
    Record(CanonError),
    /// A record contains a raw newline, or an end record is not of type end / partial.
    Shape(String),
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WriteError::Io(e) => write!(f, "I/O: {e}"),
            WriteError::Record(e) => write!(f, "record: {e}"),
            WriteError::Shape(m) => write!(f, "record shape: {m}"),
        }
    }
}
impl std::error::Error for WriteError {}
impl From<io::Error> for WriteError {
    fn from(e: io::Error) -> Self {
        WriteError::Io(e)
    }
}

/// Append-only JSONL writer: every record is written as one `write_all` of the line + "\n" (no user-space buffer, so
/// a crash leaves at most one torn last line), and the raw BLAKE3, the canonical digest, the byte, record and rule
/// counts are kept running, so a commit point needs no re-read.
pub struct RecordWriter {
    file: File,
    raw: blake3::Hasher,
    canon: CanonicalDigest,
    bytes: u64,
    records: u64,
    rules: u64,
}

impl RecordWriter {
    /// Create (truncate) `path`.
    pub fn create(path: &Path) -> io::Result<RecordWriter> {
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        Ok(RecordWriter {
            file,
            raw: blake3::Hasher::new(),
            canon: CanonicalDigest::new(),
            bytes: 0,
            records: 0,
            rules: 0,
        })
    }

    /// Resume: create `dst` holding exactly the first `committed_bytes` bytes of `src`, verify that their BLAKE3 is
    /// `prefix_blake3`, and continue with the running state of that prefix (3.8.2). The prefix must end at a line
    /// boundary.
    pub fn resume_from_prefix(
        src: &Path,
        committed_bytes: u64,
        prefix_blake3: &str,
        dst: &Path,
    ) -> Result<RecordWriter, WriteError> {
        let mut w = RecordWriter::create(dst)?;
        let mut r = BufReader::with_capacity(1 << 20, File::open(src)?.take(committed_bytes));
        let mut line = Vec::new();
        loop {
            line.clear();
            let n = r.read_until(b'\n', &mut line)?;
            if n == 0 {
                break;
            }
            if line.last() != Some(&b'\n') {
                return Err(WriteError::Shape(
                    "committed prefix does not end at a line boundary".into(),
                ));
            }
            w.push_raw_line(&line)?;
        }
        if w.bytes != committed_bytes {
            return Err(WriteError::Shape(format!(
                "source has {} bytes, prefix needs {committed_bytes}",
                w.bytes
            )));
        }
        if w.raw.finalize().to_hex().as_str() != prefix_blake3 {
            return Err(WriteError::Shape("committed prefix digest mismatch".into()));
        }
        w.file.sync_data()?;
        Ok(w)
    }

    fn push_raw_line(&mut self, line_nl: &[u8]) -> Result<RecordPos, WriteError> {
        let body = &line_nl[..line_nl.len() - 1];
        let canon = canon::canonical_line(body).map_err(WriteError::Record)?;
        let ty = canon::line_type(body).map_err(WriteError::Record)?;
        let offset = self.bytes;
        self.file.write_all(line_nl)?;
        self.raw.update(line_nl);
        let mut h = blake3::Hasher::new();
        h.update(&canon);
        h.update(b"\n");
        self.canon.push_line(body).map_err(WriteError::Record)?;
        self.bytes += line_nl.len() as u64;
        self.records += 1;
        if ty.as_deref() == Some("rule") {
            self.rules += 1;
        }
        Ok(RecordPos {
            offset,
            bytes: line_nl.len() as u64,
            canon_blake3: h.finalize().to_hex().to_string(),
        })
    }

    /// Append one record (a JSON object without the trailing newline). The record is validated (it must have a
    /// canonical form) BEFORE anything is written.
    pub fn append(&mut self, record: &[u8]) -> Result<RecordPos, WriteError> {
        if record.contains(&b'\n') {
            return Err(WriteError::Shape("record contains a newline".into()));
        }
        let mut line = Vec::with_capacity(record.len() + 1);
        line.extend_from_slice(record);
        line.push(b'\n');
        self.push_raw_line(&line)
    }

    /// Commit point: data on disk (`sync_data`) and the running prefix state.
    pub fn commit(&mut self) -> io::Result<CommitPoint> {
        self.file.sync_data()?;
        Ok(self.state())
    }

    /// The running state without syncing.
    pub fn state(&self) -> CommitPoint {
        CommitPoint {
            bytes: self.bytes,
            prefix_blake3: self.raw.finalize().to_hex().to_string(),
            records: self.records,
            rules: self.rules,
        }
    }

    /// Append the end record (`type` must be `end` or `partial`), fsync, and return the seal.
    pub fn seal(mut self, end_record: &[u8]) -> Result<TableSeal, WriteError> {
        let ty = canon::line_type(end_record).map_err(WriteError::Record)?;
        let Some(end) = ty.as_deref().and_then(EndKind::from_type) else {
            return Err(WriteError::Shape(format!(
                "end record type {ty:?} is not end / partial"
            )));
        };
        self.append(end_record)?;
        self.file.sync_all()?;
        let s = self.state();
        Ok(TableSeal {
            bytes: s.bytes,
            blake3: s.prefix_blake3,
            records: s.records,
            rules: s.rules,
            end,
            canonical_blake3: self.canon.hex(),
        })
    }

    /// The underlying file, for fault injection in tests and the fake child only (writing through it desynchronises
    /// the running digests on purpose).
    pub fn raw_file(&mut self) -> &mut File {
        &mut self.file
    }
}

/// Compute the seal of an existing table file (memory bounded by the longest line). Fails when the file is torn or
/// its last record is not an end record.
pub fn seal_file(path: &Path) -> Result<TableSeal, WriteError> {
    let mut r = BufReader::with_capacity(1 << 20, File::open(path)?);
    let mut raw = blake3::Hasher::new();
    let mut canon = CanonicalDigest::new();
    let (mut bytes, mut records, mut rules) = (0u64, 0u64, 0u64);
    let mut last_type = None;
    let mut line = Vec::new();
    loop {
        line.clear();
        let n = r.read_until(b'\n', &mut line)?;
        if n == 0 {
            break;
        }
        if line.last() != Some(&b'\n') {
            return Err(WriteError::Record(CanonError::TornLine {
                line: records + 1,
            }));
        }
        raw.update(&line);
        canon.push_line(&line).map_err(WriteError::Record)?;
        last_type = canon::line_type(&line).map_err(WriteError::Record)?;
        if last_type.as_deref() == Some("rule") {
            rules += 1;
        }
        bytes += n as u64;
        records += 1;
    }
    let Some(end) = last_type.as_deref().and_then(EndKind::from_type) else {
        return Err(WriteError::Shape(format!(
            "last record type {last_type:?} is not end / partial"
        )));
    };
    Ok(TableSeal {
        bytes,
        blake3: raw.finalize().to_hex().to_string(),
        records,
        rules,
        end,
        canonical_blake3: canon.hex(),
    })
}

/// BLAKE3 of the first `n` bytes of a file; `None` if the file is shorter. Constant memory.
pub fn prefix_blake3(path: &Path, n: u64) -> io::Result<Option<String>> {
    let f = File::open(path)?;
    let mut r = f.take(n);
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut got = 0u64;
    loop {
        let k = r.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        got += k as u64;
    }
    Ok((got == n).then(|| h.finalize().to_hex().to_string()))
}

/// BLAKE3 and size of a whole file, constant memory.
pub fn file_blake3(path: &Path) -> io::Result<(u64, String)> {
    let mut f = File::open(path)?;
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut n = 0u64;
    loop {
        let k = f.read(&mut buf)?;
        if k == 0 {
            break;
        }
        h.update(&buf[..k]);
        n += k as u64;
    }
    Ok((n, h.finalize().to_hex().to_string()))
}

/// Why a table is not sealed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsealReason {
    /// OUTCOME says `sealed: false` (e.g. a hard-safe stop).
    OutcomeNotSealed,
    /// OUTCOME says sealed but lacks `bytes` or `blake3`.
    OutcomeIncomplete,
    /// The table file cannot be opened / read.
    FileUnreadable(String),
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    DigestMismatch,
    Empty,
    /// The file does not end with "\n".
    TornLastLine,
    /// The last line is longer than [`END_RECORD_MAX`].
    LastLineTooLong,
    /// The last line is not an end record (its type, if any).
    NoEndRecord(Option<String>),
    /// The last line is an end record of the other kind than OUTCOME says.
    EndMismatch {
        outcome: EndKind,
        file: EndKind,
    },
    RecordCountMismatch {
        expected: u64,
        actual: u64,
    },
}

impl fmt::Display for UnsealReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Verdict of [`verify_sealed`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealVerdict {
    Sealed,
    Unsealed(UnsealReason),
}

impl SealVerdict {
    pub fn is_sealed(&self) -> bool {
        matches!(self, SealVerdict::Sealed)
    }
}

/// Verify a table file against its OUTCOME entry. Streaming, constant memory (1 MiB read buffer + at most
/// [`END_RECORD_MAX`] bytes of the last line), robust to lines of any length.
pub fn verify_sealed(table: &Path, out: &TableOutput) -> SealVerdict {
    use SealVerdict::Unsealed;
    if !out.sealed {
        return Unsealed(UnsealReason::OutcomeNotSealed);
    }
    let (Some(bytes), Some(digest)) = (out.bytes, out.blake3.as_deref()) else {
        return Unsealed(UnsealReason::OutcomeIncomplete);
    };
    let mut f = match File::open(table) {
        Ok(f) => f,
        Err(e) => return Unsealed(UnsealReason::FileUnreadable(e.to_string())),
    };
    let mut h = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 20];
    let mut size = 0u64;
    let mut lines = 0u64;
    // `cur`: the line being read (bounded capture); `prev`: the last completed line (bounded capture)
    let (mut cur, mut cur_long) = (Vec::new(), false);
    let (mut prev, mut prev_long) = (Vec::new(), false);
    let mut last_byte = None;
    fn take(dst: &mut Vec<u8>, long: &mut bool, piece: &[u8]) {
        if *long {
            return;
        }
        if dst.len() + piece.len() > END_RECORD_MAX {
            *long = true;
            *dst = Vec::new();
        } else {
            dst.extend_from_slice(piece);
        }
    }
    loop {
        let k = match f.read(&mut buf) {
            Ok(0) => break,
            Ok(k) => k,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Unsealed(UnsealReason::FileUnreadable(e.to_string())),
        };
        let chunk = &buf[..k];
        h.update(chunk);
        size += k as u64;
        last_byte = chunk.last().copied();
        let mut s = 0;
        for (i, &b) in chunk.iter().enumerate() {
            if b == b'\n' {
                take(&mut cur, &mut cur_long, &chunk[s..i]);
                std::mem::swap(&mut prev, &mut cur);
                prev_long = cur_long;
                cur.clear();
                cur_long = false;
                lines += 1;
                s = i + 1;
            }
        }
        take(&mut cur, &mut cur_long, &chunk[s..]);
    }
    if size != bytes {
        return Unsealed(UnsealReason::SizeMismatch {
            expected: bytes,
            actual: size,
        });
    }
    if h.finalize().to_hex().as_str() != digest {
        return Unsealed(UnsealReason::DigestMismatch);
    }
    if size == 0 {
        return Unsealed(UnsealReason::Empty);
    }
    if last_byte != Some(b'\n') {
        return Unsealed(UnsealReason::TornLastLine);
    }
    if prev_long {
        return Unsealed(UnsealReason::LastLineTooLong);
    }
    let ty = match canon::line_type(&prev) {
        Ok(t) => t,
        Err(_) => return Unsealed(UnsealReason::NoEndRecord(None)),
    };
    let Some(end) = ty.as_deref().and_then(EndKind::from_type) else {
        return Unsealed(UnsealReason::NoEndRecord(ty));
    };
    if let Some(o) = out.end
        && o != end
    {
        return Unsealed(UnsealReason::EndMismatch {
            outcome: o,
            file: end,
        });
    }
    if let Some(r) = out.records
        && r != lines
    {
        return Unsealed(UnsealReason::RecordCountMismatch {
            expected: r,
            actual: lines,
        });
    }
    SealVerdict::Sealed
}
