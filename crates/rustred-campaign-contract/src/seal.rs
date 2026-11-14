//! Append-only record streams with running digests, sealing, and the sealed-stream VERIFIER (child-job.md 3.4.4,
//! 3.9; ruling R-b: records are accepted ONLY from a sealed, verified table of THIS attempt).
//!
//! Revised by audits A/B (C1, C2, M2, M3, M4, m6, m7, F4, F7, F10). A stream is SEALED iff
//! - its path is the LAYOUT path inside the run directory (never a path read from OUTCOME), reached without any
//!   symlink, opened with O_NOFOLLOW, a regular file whose (dev, inode) equals the directory entry's;
//! - every line is a record with a canonical form ([`crate::canon`]), the file ends with "\n", exactly one end
//!   record exists and it is the LAST line (no interior end record, no byte after it);
//! - rule records (`type` = `rule`) have unique (`chart`, `index`) and unique (`chart`, `target`), `index` an integer;
//! - every figure OUTCOME reports about it (size, BLAKE3, records, rules, end kind, canonical digest, elided witness
//!   loci) equals the figure recomputed here.
//!
//! The verifier also counts elided witness loci itself ([`crate::fake::ELIDED`] inside a top-level `witness_loci`
//! array of any record); the child's own count is never trusted. Memory: a small multiple of the longest line.

use crate::canon::{self, CanonError, CanonicalDigest};
use crate::layout::RunLayout;
use crate::outcome::{EndKind, TableOutput};
use crate::resume::CommittedFile;
use std::collections::HashSet;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Component, Path, PathBuf};

/// Kept for callers: an end record longer than this is suspicious (no longer a verifier limit).
pub const END_RECORD_MAX: usize = 1 << 20;

/// `O_NOFOLLOW` of the Linux targets (the crate has no libc, ruling R-a); 0 elsewhere, where the lstat + (dev, ino)
/// comparison alone guards against a swapped symlink.
#[cfg(all(target_os = "linux", any(target_arch = "x86_64", target_arch = "x86")))]
pub const O_NOFOLLOW: i32 = 0o400000;
#[cfg(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm")))]
pub const O_NOFOLLOW: i32 = 0o100000;
#[cfg(not(all(
    target_os = "linux",
    any(
        target_arch = "x86_64",
        target_arch = "x86",
        target_arch = "aarch64",
        target_arch = "arm"
    )
)))]
pub const O_NOFOLLOW: i32 = 0;

/// End record types of the table (and of the other JSONL result streams).
pub const TABLE_END_TYPES: [&str; 2] = ["end", "partial"];
/// End record types of the cert stream.
pub const CERT_END_TYPES: [&str; 2] = ["cert_end", "cert_partial"];

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
    /// Running canonical digest of the committed records.
    pub canonical_blake3: String,
}

impl CommitPoint {
    /// The RESUME form of this commit point for the stream at layout path `path`.
    pub fn committed(&self, path: &str) -> CommittedFile {
        CommittedFile {
            path: path.to_string(),
            bytes: self.bytes,
            blake3: self.prefix_blake3.clone(),
            records: self.records,
            rules: self.rules,
            canonical_blake3: self.canonical_blake3.clone(),
        }
    }
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
    /// A record contains a raw newline, an end record is not of type end / partial, or a resume prefix is invalid.
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

/// Why a stream is not sealed (or a prefix not valid).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UnsealReason {
    /// OUTCOME says `sealed: false` (e.g. a hard-safe stop).
    OutcomeNotSealed,
    /// OUTCOME says sealed but lacks a figure (bytes, blake3, records, rules, end, canonical digest).
    OutcomeIncomplete,
    /// OUTCOME does not list an output the kind's sealing rule requires.
    MissingOutput(String),
    /// The path recorded in OUTCOME is not the layout path (a contract violation, C2).
    PathNotLayout(String),
    /// A component of the path is a symlink or not a directory, or the file is not a regular file.
    NotRegularFile(String),
    /// The opened file is not the one the directory entry names.
    InodeChanged,
    /// The file cannot be opened / read.
    FileUnreadable(String),
    SizeMismatch {
        expected: u64,
        actual: u64,
    },
    DigestMismatch,
    Empty,
    /// The file does not end with "\n".
    TornLastLine,
    /// A line without a canonical form (not JSON, duplicate key, non-canonical number...).
    BadRecord {
        line: u64,
        why: String,
    },
    /// An end record before the last line.
    InteriorEndRecord {
        line: u64,
    },
    /// The last line is not an end record (its type, if any).
    NoEndRecord(Option<String>),
    /// The end record is of another kind than OUTCOME says.
    EndMismatch {
        outcome: String,
        file: String,
    },
    RecordCountMismatch {
        expected: u64,
        actual: u64,
    },
    RuleCountMismatch {
        expected: u64,
        actual: u64,
    },
    CanonicalDigestMismatch,
    ElidedCountMismatch {
        outcome: u64,
        file: u64,
    },
    /// A rule record without an integer `index`.
    BadRuleIndex {
        line: u64,
    },
    /// Two rule records with the same (`chart`, `index`).
    DuplicateRuleIndex {
        line: u64,
    },
    /// Two rule records with the same (`chart`, `target`).
    DuplicateRuleTarget {
        line: u64,
    },
    /// A resume prefix contains an end record.
    EndRecordInPrefix {
        line: u64,
    },
    /// A resume prefix does not end at a line boundary.
    PrefixNotAtLineBoundary,
}

impl fmt::Display for UnsealReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// What the verifier recomputed for one stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedStream {
    /// The layout path relative to the run directory.
    pub rel: String,
    pub dev: u64,
    pub ino: u64,
    pub bytes: u64,
    pub blake3: String,
    pub records: u64,
    pub rules: u64,
    /// Type of the end record.
    pub end: String,
    pub canonical_blake3: String,
    /// Elided witness loci found by the verifier (R-f, M2).
    pub elided_loci: u64,
    /// Records with at least one elided locus.
    pub elided_records: u64,
    /// `family_blake3` of a header record on the first line, if any (decoded).
    pub family_blake3: Option<String>,
}

/// Verdict of [`verify_table`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SealVerdict {
    Sealed(Box<VerifiedStream>),
    Unsealed(UnsealReason),
}

impl SealVerdict {
    pub fn is_sealed(&self) -> bool {
        matches!(self, SealVerdict::Sealed(_))
    }
}

/// Line-by-line accumulator shared by the writer (resume copy), the verifier and the prefix check.
struct Scan<'a> {
    end_types: &'a [&'a str],
    raw: blake3::Hasher,
    canon: CanonicalDigest,
    bytes: u64,
    records: u64,
    rules: u64,
    ids: HashSet<(Vec<u8>, Vec<u8>)>,
    targets: HashSet<(Vec<u8>, Vec<u8>)>,
    elided_loci: u64,
    elided_records: u64,
    end: Option<String>,
    family: Option<String>,
}

impl<'a> Scan<'a> {
    fn new(end_types: &'a [&'a str]) -> Scan<'a> {
        Scan {
            end_types,
            raw: blake3::Hasher::new(),
            canon: CanonicalDigest::new(),
            bytes: 0,
            records: 0,
            rules: 0,
            ids: HashSet::new(),
            targets: HashSet::new(),
            elided_loci: 0,
            elided_records: 0,
            end: None,
            family: None,
        }
    }

    /// One complete line including its "\n". Returns the canonical record.
    fn push(&mut self, line_nl: &[u8]) -> Result<canon::CanonRecord, UnsealReason> {
        let line_no = self.records + 1;
        if self.end.is_some() {
            return Err(UnsealReason::InteriorEndRecord { line: self.records });
        }
        let body = &line_nl[..line_nl.len() - 1];
        let c = canon::canonical_record(body).map_err(|e| UnsealReason::BadRecord {
            line: line_no,
            why: e.to_string(),
        })?;
        let ty = c.record_type.as_deref();
        if ty.is_some_and(|t| self.end_types.contains(&t)) {
            self.end = ty.map(str::to_string);
        }
        if ty == Some("rule") {
            let chart = c.member("chart").unwrap_or(b"null").to_vec();
            let index = c.member("index").unwrap_or(b"");
            if index.is_empty() || !index.iter().all(|b| b.is_ascii_digit() || *b == b'-') {
                return Err(UnsealReason::BadRuleIndex { line: line_no });
            }
            if !self.ids.insert((chart.clone(), index.to_vec())) {
                return Err(UnsealReason::DuplicateRuleIndex { line: line_no });
            }
            if let Some(t) = c.member("target")
                && !self.targets.insert((chart, t.to_vec()))
            {
                return Err(UnsealReason::DuplicateRuleTarget { line: line_no });
            }
            self.rules += 1;
        }
        if let Some(l) = c.member("witness_loci") {
            let n = canon::count_string_elements(l, crate::fake::ELIDED).unwrap_or(0);
            self.elided_loci += n;
            self.elided_records += u64::from(n > 0);
        }
        if self.records == 0 && ty == Some("header") {
            self.family = c
                .member(canon::FAMILY_DIGEST_KEY)
                .and_then(canon::decode_canonical_string);
        }
        self.raw.update(line_nl);
        self.canon.push_canonical(&c.bytes);
        self.bytes += line_nl.len() as u64;
        self.records += 1;
        Ok(c)
    }

    /// Feed a whole reader line by line; `Err(TornLastLine)` for bytes after the last "\n".
    fn feed<Rd: BufRead>(
        &mut self,
        mut r: Rd,
        mut each: impl FnMut(&[u8]) -> io::Result<()>,
    ) -> Result<(), UnsealReason> {
        let mut line = Vec::new();
        loop {
            line.clear();
            let n = r
                .read_until(b'\n', &mut line)
                .map_err(|e| UnsealReason::FileUnreadable(e.to_string()))?;
            if n == 0 {
                return Ok(());
            }
            if line.last() != Some(&b'\n') {
                return Err(UnsealReason::TornLastLine);
            }
            self.push(&line)?;
            each(&line).map_err(|e| UnsealReason::FileUnreadable(e.to_string()))?;
        }
    }

    fn committed(&self, path: &str) -> CommittedFile {
        CommittedFile {
            path: path.to_string(),
            bytes: self.bytes,
            blake3: self.raw.finalize().to_hex().to_string(),
            records: self.records,
            rules: self.rules,
            canonical_blake3: self.canon.hex(),
        }
    }

    /// Compare with the figures of a committed prefix (no end record allowed in it).
    fn check_prefix(&self, want: &CommittedFile) -> Result<(), UnsealReason> {
        if self.end.is_some() {
            return Err(UnsealReason::EndRecordInPrefix { line: self.records });
        }
        let got = self.committed(&want.path);
        if got.bytes != want.bytes {
            return Err(UnsealReason::SizeMismatch {
                expected: want.bytes,
                actual: got.bytes,
            });
        }
        if got.blake3 != want.blake3 {
            return Err(UnsealReason::DigestMismatch);
        }
        if got.records != want.records {
            return Err(UnsealReason::RecordCountMismatch {
                expected: want.records,
                actual: got.records,
            });
        }
        if got.rules != want.rules {
            return Err(UnsealReason::RuleCountMismatch {
                expected: want.rules,
                actual: got.rules,
            });
        }
        if got.canonical_blake3 != want.canonical_blake3 {
            return Err(UnsealReason::CanonicalDigestMismatch);
        }
        Ok(())
    }
}

/// Validate a layout path relative to the run directory: only normal components (no root, `.`, `..`).
fn rel_ok(rel: &str) -> bool {
    let p = Path::new(rel);
    !rel.is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

/// Open `root/rel` safely (C2, F7): every component below `root` is a real directory (no symlink), the file is a
/// regular file, it is opened with O_NOFOLLOW, and its (dev, inode) equals the directory entry's.
pub fn open_in_run(root: &Path, rel: &str) -> Result<(File, u64, u64), UnsealReason> {
    if !rel_ok(rel) {
        return Err(UnsealReason::PathNotLayout(rel.to_string()));
    }
    let mut cur = root.to_path_buf();
    let comps: Vec<_> = Path::new(rel).components().collect();
    for (k, c) in comps.iter().enumerate() {
        cur.push(c.as_os_str());
        let md = std::fs::symlink_metadata(&cur)
            .map_err(|e| UnsealReason::FileUnreadable(format!("{}: {e}", cur.display())))?;
        let last = k + 1 == comps.len();
        let ok = if last {
            md.file_type().is_file()
        } else {
            md.file_type().is_dir()
        };
        if !ok {
            return Err(UnsealReason::NotRegularFile(cur.display().to_string()));
        }
    }
    let entry =
        std::fs::symlink_metadata(&cur).map_err(|e| UnsealReason::FileUnreadable(e.to_string()))?;
    let f = OpenOptions::new()
        .read(true)
        .custom_flags(O_NOFOLLOW)
        .open(&cur)
        .map_err(|e| UnsealReason::FileUnreadable(format!("{}: {e}", cur.display())))?;
    let md = f
        .metadata()
        .map_err(|e| UnsealReason::FileUnreadable(e.to_string()))?;
    if !md.file_type().is_file() || md.dev() != entry.dev() || md.ino() != entry.ino() {
        return Err(UnsealReason::InodeChanged);
    }
    Ok((f, md.dev(), md.ino()))
}

/// Verify the JSONL stream at layout path `rel` of the run directory `root` (structure only; compare the figures
/// with OUTCOME's claims with [`compare_table`] / [`compare_figures`]).
pub fn verify_stream(
    root: &Path,
    rel: &str,
    end_types: &[&str],
) -> Result<VerifiedStream, UnsealReason> {
    let (f, dev, ino) = open_in_run(root, rel)?;
    let mut s = Scan::new(end_types);
    s.feed(BufReader::with_capacity(1 << 20, f), |_| Ok(()))?;
    if s.records == 0 {
        return Err(UnsealReason::Empty);
    }
    let Some(end) = s.end.clone() else {
        return Err(UnsealReason::NoEndRecord(None));
    };
    Ok(VerifiedStream {
        rel: rel.to_string(),
        dev,
        ino,
        bytes: s.bytes,
        blake3: s.raw.finalize().to_hex().to_string(),
        records: s.records,
        rules: s.rules,
        end,
        canonical_blake3: s.canon.hex(),
        elided_loci: s.elided_loci,
        elided_records: s.elided_records,
        family_blake3: s.family,
    })
}

/// Compare generic OUTCOME figures with a verified stream; `None` figures are missing claims (incomplete).
pub fn compare_figures(
    v: &VerifiedStream,
    bytes: Option<u64>,
    blake3: Option<&str>,
    records: Option<u64>,
    canonical_blake3: Option<&str>,
) -> Result<(), UnsealReason> {
    let (Some(bytes), Some(blake3), Some(records), Some(canonical)) =
        (bytes, blake3, records, canonical_blake3)
    else {
        return Err(UnsealReason::OutcomeIncomplete);
    };
    if bytes != v.bytes {
        return Err(UnsealReason::SizeMismatch {
            expected: bytes,
            actual: v.bytes,
        });
    }
    if blake3 != v.blake3 {
        return Err(UnsealReason::DigestMismatch);
    }
    if records != v.records {
        return Err(UnsealReason::RecordCountMismatch {
            expected: records,
            actual: v.records,
        });
    }
    if canonical != v.canonical_blake3 {
        return Err(UnsealReason::CanonicalDigestMismatch);
    }
    Ok(())
}

/// Every figure of OUTCOME's table entry against the verified table: size, BLAKE3, records, rules (= rules_committed),
/// end kind, canonical digest, elided loci.
pub fn compare_table(t: &TableOutput, v: &VerifiedStream) -> Result<(), UnsealReason> {
    if !t.sealed {
        return Err(UnsealReason::OutcomeNotSealed);
    }
    compare_figures(
        v,
        t.bytes,
        t.blake3.as_deref(),
        t.records,
        t.canonical_blake3.as_deref(),
    )?;
    let (Some(rules), Some(end)) = (t.rules, t.end) else {
        return Err(UnsealReason::OutcomeIncomplete);
    };
    if rules != v.rules || t.rules_committed != v.rules {
        return Err(UnsealReason::RuleCountMismatch {
            expected: rules,
            actual: v.rules,
        });
    }
    if end.as_str() != v.end {
        return Err(UnsealReason::EndMismatch {
            outcome: end.as_str().into(),
            file: v.end.clone(),
        });
    }
    if t.witness_loci_elided != v.elided_loci {
        return Err(UnsealReason::ElidedCountMismatch {
            outcome: t.witness_loci_elided,
            file: v.elided_loci,
        });
    }
    Ok(())
}

/// The table of a run: the file at the LAYOUT path (`out/F.table.jsonl`), never the path OUTCOME names (which must
/// equal it), verified and compared with every OUTCOME figure.
pub fn verify_table(layout: &RunLayout, t: &TableOutput) -> SealVerdict {
    let rel = RunLayout::out_rel(crate::layout::TABLE);
    if t.path != rel {
        return SealVerdict::Unsealed(UnsealReason::PathNotLayout(t.path.clone()));
    }
    if !t.sealed {
        return SealVerdict::Unsealed(UnsealReason::OutcomeNotSealed);
    }
    match verify_stream(&layout.root, &rel, &TABLE_END_TYPES)
        .and_then(|v| compare_table(t, &v).map(|_| v))
    {
        Ok(v) => SealVerdict::Sealed(Box::new(v)),
        Err(r) => SealVerdict::Unsealed(r),
    }
}

/// Check that the first `want.bytes` bytes of the stream at `root/want.path` are exactly the committed prefix
/// `want` (bytes, digest, records, rules, canonical digest, line boundary, no end record, unique rule identities).
pub fn verify_prefix(
    root: &Path,
    want: &CommittedFile,
    end_types: &[&str],
) -> Result<(), UnsealReason> {
    let (f, _, _) = open_in_run(root, &want.path)?;
    let mut s = Scan::new(end_types);
    match s.feed(
        BufReader::with_capacity(1 << 20, f.take(want.bytes)),
        |_| Ok(()),
    ) {
        Err(UnsealReason::TornLastLine) => return Err(UnsealReason::PrefixNotAtLineBoundary),
        r => r?,
    }
    s.check_prefix(want)
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
    /// Create `path` (it must not exist: O_EXCL, which also refuses a planted symlink) and fsync its directory (m6).
    pub fn create(path: &Path) -> io::Result<RecordWriter> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(O_NOFOLLOW)
            .open(path)?;
        if let Some(d) = path.parent() {
            crate::atomic::fsync_dir(d)?;
        }
        Ok(RecordWriter {
            file,
            raw: blake3::Hasher::new(),
            canon: CanonicalDigest::new(),
            bytes: 0,
            records: 0,
            rules: 0,
        })
    }

    /// Resume (3.8.2, C1, m7): create `dst` holding exactly the committed prefix `want` of the previous attempt's
    /// stream in run directory `src_root` (layout path `want.path`), verifying every figure WHILE copying, and
    /// continue with its running state. The copy goes to a temporary name that is renamed only after the whole
    /// prefix verified, so on any mismatch nothing is left at `dst`. The source must not be `dst` itself.
    pub fn resume_from(
        src_root: &Path,
        want: &CommittedFile,
        end_types: &[&str],
        dst: &Path,
    ) -> Result<RecordWriter, WriteError> {
        let (src, dev, ino) =
            open_in_run(src_root, &want.path).map_err(|r| WriteError::Shape(r.to_string()))?;
        if let Ok(m) = std::fs::symlink_metadata(dst)
            && m.dev() == dev
            && m.ino() == ino
        {
            return Err(WriteError::Shape(
                "resume source and destination are the same file".into(),
            ));
        }
        let tmp = crate::atomic::temp_path(dst);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .custom_flags(O_NOFOLLOW)
            .open(&tmp)?;
        let mut s = Scan::new(end_types);
        let r = (|| {
            let res = s.feed(
                BufReader::with_capacity(1 << 20, src.take(want.bytes)),
                |l| file.write_all(l),
            );
            match res {
                Err(UnsealReason::TornLastLine) => {
                    return Err(WriteError::Shape(
                        UnsealReason::PrefixNotAtLineBoundary.to_string(),
                    ));
                }
                Err(e) => return Err(WriteError::Shape(e.to_string())),
                Ok(()) => {}
            }
            s.check_prefix(want)
                .map_err(|e| WriteError::Shape(e.to_string()))?;
            file.sync_all()?;
            std::fs::rename(&tmp, dst)?;
            if let Some(d) = dst.parent() {
                crate::atomic::fsync_dir(d)?;
            }
            Ok(())
        })();
        if let Err(e) = r {
            let _ = std::fs::remove_file(&tmp);
            return Err(e);
        }
        Ok(RecordWriter {
            file,
            raw: s.raw,
            canon: s.canon,
            bytes: s.bytes,
            records: s.records,
            rules: s.rules,
        })
    }

    fn push_raw_line(&mut self, line_nl: &[u8]) -> Result<RecordPos, WriteError> {
        let body = &line_nl[..line_nl.len() - 1];
        let c = canon::canonical_record(body).map_err(WriteError::Record)?;
        let offset = self.bytes;
        self.file.write_all(line_nl)?;
        self.raw.update(line_nl);
        let mut h = blake3::Hasher::new();
        h.update(&c.bytes);
        h.update(b"\n");
        self.canon.push_canonical(&c.bytes);
        self.bytes += line_nl.len() as u64;
        self.records += 1;
        if c.record_type.as_deref() == Some("rule") {
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
            canonical_blake3: self.canon.hex(),
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
            canonical_blake3: s.canonical_blake3,
        })
    }

    /// The underlying file, for fault injection in tests only (writing through it desynchronises the running digests
    /// on purpose).
    pub fn raw_file(&mut self) -> &mut File {
        &mut self.file
    }
}

/// Compute the seal of an existing table file with the verifier's rules (any path; for tests and tools).
pub fn seal_file(path: &Path) -> Result<TableSeal, WriteError> {
    let f = File::open(path)?;
    let mut s = Scan::new(&TABLE_END_TYPES);
    s.feed(BufReader::with_capacity(1 << 20, f), |_| Ok(()))
        .map_err(|e| WriteError::Shape(e.to_string()))?;
    let Some(end) = s.end.as_deref().and_then(EndKind::from_type) else {
        return Err(WriteError::Shape("no end record".into()));
    };
    let c = s.committed("");
    Ok(TableSeal {
        bytes: c.bytes,
        blake3: c.blake3,
        records: c.records,
        rules: c.rules,
        end,
        canonical_blake3: c.canonical_blake3,
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

/// The path of a layout file of a run (helper for callers that print it).
pub fn layout_path(root: &Path, rel: &str) -> PathBuf {
    root.join(rel)
}

/// Running digest of a stream written by someone else in arbitrary chunks (the engine's `BufWriter` flushes): the
/// engine-side twin of [`RecordWriter`] (child-job.md 3.9 "the table writer feeds a running BLAKE3 at flush level",
/// change C7). Bytes are fed exactly as they reach the file; complete lines are checked with the verifier's rules
/// (canonical form, end record last, unique rule identities, elided witness loci, header `family_blake3`), so the
/// figures at a line boundary are those [`verify_table`] / [`verify_prefix`] will recompute from the file.
/// The first record that fails a rule makes the digest FAILED (sticky): the stream can then never be sealed or
/// committed, which is the fail-closed answer (the bytes are still written by the caller).
pub struct StreamDigest {
    scan: Scan<'static>,
    partial: Vec<u8>,
    failed: Option<String>,
    records: Vec<RecordInfo>,
    record_types: bool,
}

/// One complete record seen by a [`StreamDigest`] that records them: its `type`, offset, line length (with "\n")
/// and the BLAKE3 of its canonical line + "\n" (the `rule_committed.canon_blake3` of child-job.md 3.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordInfo {
    pub record_type: Option<String>,
    pub offset: u64,
    pub bytes: u64,
    pub canon_blake3: String,
}

impl StreamDigest {
    /// A digest for a stream whose end record types are `end_types` (e.g. [`TABLE_END_TYPES`], [`CERT_END_TYPES`]).
    pub fn new(end_types: &'static [&'static str]) -> StreamDigest {
        StreamDigest {
            scan: Scan::new(end_types),
            partial: Vec::new(),
            failed: None,
            records: Vec::new(),
            record_types: false,
        }
    }

    /// Also remember every complete record ([`RecordInfo`]) until [`StreamDigest::take_records`] (the engine derives
    /// a case's result from the queue records of that case and reports the rule lines it committed).
    pub fn recording_types(mut self) -> StreamDigest {
        self.record_types = true;
        self
    }

    /// Feed bytes exactly as written to the file.
    pub fn feed(&mut self, mut chunk: &[u8]) {
        while let Some(at) = chunk.iter().position(|&b| b == b'\n') {
            let (head, rest) = chunk.split_at(at + 1);
            chunk = rest;
            if self.partial.is_empty() {
                self.line(head);
            } else {
                let mut l = std::mem::take(&mut self.partial);
                l.extend_from_slice(head);
                self.line(&l);
            }
        }
        self.partial.extend_from_slice(chunk);
    }

    fn line(&mut self, line_nl: &[u8]) {
        if self.failed.is_some() {
            // keep the raw size right for diagnostics; no figure is reported once failed
            self.scan.bytes += line_nl.len() as u64;
            return;
        }
        let offset = self.scan.bytes;
        match self.scan.push(line_nl) {
            Ok(c) => {
                if self.record_types {
                    let mut h = blake3::Hasher::new();
                    h.update(&c.bytes);
                    h.update(b"\n");
                    self.records.push(RecordInfo {
                        record_type: c.record_type,
                        offset,
                        bytes: line_nl.len() as u64,
                        canon_blake3: h.finalize().to_hex().to_string(),
                    });
                }
            }
            Err(e) => {
                self.scan.bytes += line_nl.len() as u64;
                self.failed = Some(e.to_string());
            }
        }
    }

    /// Whether the bytes fed so far end at a line boundary.
    pub fn at_boundary(&self) -> bool {
        self.partial.is_empty()
    }

    /// Why the stream can no longer be committed or sealed (first failing record), if so.
    pub fn failed(&self) -> Option<&str> {
        self.failed.as_deref()
    }

    /// Bytes fed so far (including a pending partial line).
    pub fn bytes_fed(&self) -> u64 {
        self.scan.bytes + self.partial.len() as u64
    }

    /// The end record type seen, if any.
    pub fn end(&self) -> Option<&str> {
        self.scan.end.as_deref()
    }

    /// Rule records seen.
    pub fn rules(&self) -> u64 {
        self.scan.rules
    }

    /// Elided witness loci counted (same rule as the verifier).
    pub fn elided_loci(&self) -> u64 {
        self.scan.elided_loci
    }

    /// `family_blake3` of a header first record, if present.
    pub fn family_blake3(&self) -> Option<&str> {
        self.scan.family.as_deref()
    }

    /// The figures of the complete records fed so far, as a committed prefix for the layout path `path`: `None`
    /// when a partial line is pending, the digest failed, or an end record was seen (a prefix never holds one).
    pub fn committed(&self, path: &str) -> Option<CommittedFile> {
        (self.at_boundary() && self.failed.is_none() && self.scan.end.is_none())
            .then(|| self.scan.committed(path))
    }

    /// The seal: `None` unless the stream ends exactly after its end record and no record failed.
    pub fn seal(&self) -> Option<TableSeal> {
        if !self.at_boundary() || self.failed.is_some() {
            return None;
        }
        let end = self.scan.end.as_deref().and_then(EndKind::from_type)?;
        let c = self.scan.committed("");
        Some(TableSeal {
            bytes: c.bytes,
            blake3: c.blake3,
            records: c.records,
            rules: c.rules,
            end,
            canonical_blake3: c.canonical_blake3,
        })
    }

    /// The records seen since the last call (only with [`StreamDigest::recording_types`]).
    pub fn take_records(&mut self) -> Vec<RecordInfo> {
        std::mem::take(&mut self.records)
    }

    /// Feed a whole existing file (a verified resume prefix that the caller continues to append to).
    pub fn feed_file(&mut self, path: &Path) -> io::Result<()> {
        let mut f = File::open(path)?;
        let mut buf = vec![0u8; 1 << 20];
        loop {
            let k = f.read(&mut buf)?;
            if k == 0 {
                return Ok(());
            }
            self.feed(&buf[..k]);
        }
    }
}

#[cfg(test)]
mod stream_digest_tests {
    use super::*;

    const LINES: [&str; 5] = [
        r#"{"type":"header","family_file":"/a/F.sym","family_blake3":"0000000000000000000000000000000000000000000000000000000000000000","zero_seconds":0.5}"#,
        r#"{"type":"rule","chart":"11","index":0,"target":"I(1)","rhs":[],"seconds_search":0.1,"witness_loci":["n1+1"]}"#,
        r#"{"type":"rule","chart":"11","index":1,"target":"I(2)","rhs":[],"lift_seconds":2.0,"witness_loci":["elided","n2"]}"#,
        r#"{"type":"residual","chart":"11","key":"I(3)"}"#,
        r#"{"type":"end","charts":1,"rules":2,"seconds":1.25,"vmhwm_kb":10}"#,
    ];

    fn file_of(lines: &[&str]) -> Vec<u8> {
        let mut b = Vec::new();
        for l in lines {
            b.extend_from_slice(l.as_bytes());
            b.push(b'\n');
        }
        b
    }

    #[test]
    fn chunked_feed_equals_the_file_seal_at_every_split() {
        let bytes = file_of(&LINES);
        let dir = std::env::temp_dir().join(format!("sd-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("t.jsonl");
        std::fs::write(&p, &bytes).unwrap();
        let want = seal_file(&p).unwrap();
        for step in [1usize, 2, 3, 7, 64, 4096] {
            let mut d = StreamDigest::new(&TABLE_END_TYPES);
            for c in bytes.chunks(step) {
                d.feed(c);
            }
            assert_eq!(d.seal().as_ref(), Some(&want), "step {step}");
            assert_eq!(d.elided_loci(), 1);
            assert_eq!(d.rules(), 2);
            assert!(d.committed("out/F.table.jsonl").is_none(), "end record seen");
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn committed_prefix_matches_verify_prefix() {
        let dir = std::env::temp_dir().join(format!("sdp-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("out")).unwrap();
        let bytes = file_of(&LINES[..3]);
        std::fs::write(dir.join("out/F.table.jsonl"), &bytes).unwrap();
        let mut d = StreamDigest::new(&TABLE_END_TYPES);
        d.feed(&bytes[..10]);
        assert!(d.committed("out/F.table.jsonl").is_none(), "partial line");
        d.feed(&bytes[10..]);
        let c = d.committed("out/F.table.jsonl").unwrap();
        assert_eq!(c.bytes, bytes.len() as u64);
        assert_eq!(c.rules, 2);
        verify_prefix(&dir, &c, &TABLE_END_TYPES).unwrap();
        assert_eq!(d.family_blake3().map(str::len), Some(64));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_bad_or_duplicate_record_fails_closed() {
        let mut d = StreamDigest::new(&TABLE_END_TYPES);
        d.feed(file_of(&[LINES[0], LINES[1], LINES[1]]).as_slice());
        assert!(d.failed().is_some());
        assert!(d.committed("x").is_none());
        let mut d = StreamDigest::new(&TABLE_END_TYPES);
        d.feed(b"{\"type\":\"rule\",\"index\":0,\n");
        assert!(d.failed().is_some());
        let mut d = StreamDigest::new(&TABLE_END_TYPES);
        d.feed(file_of(&[LINES[4], LINES[3]]).as_slice());
        assert!(d.seal().is_none(), "interior end record");
    }

    #[test]
    fn record_types_are_taken_in_order() {
        let mut d = StreamDigest::new(&TABLE_END_TYPES).recording_types();
        d.feed(b"{\"type\":\"covered\"}\n{\"x\":1}\n{\"type\":\"enq");
        let r = d.take_records();
        let types: Vec<_> = r.iter().map(|x| x.record_type.clone()).collect();
        assert_eq!(types, vec![Some("covered".to_string()), None]);
        assert_eq!((r[1].offset, r[1].bytes), (19, 8));
        assert_eq!(r[1].canon_blake3, crate::blake3_hex(b"{\"x\":1}\n"));
        d.feed(b"ueue\"}\n");
        assert_eq!(d.take_records()[0].record_type.as_deref(), Some("enqueue"));
        assert!(d.take_records().is_empty());
    }
}
