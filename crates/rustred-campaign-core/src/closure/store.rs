//! Compact stores of a closure run (re-audit F5, F9; lane closurefix, closure.md section 15).
//!
//! - [`ReqStore`]: fact requests as 12-byte records (key index, dispatch position, term index with the reason / image
//!   flags in its top bits) plus the term's instantiated key PACKED in the run's key codec (an explicit `i32` copy only
//!   for a term key outside the codec range). No heap allocation per request; the distinct count is computed without
//!   materialising a [`super::FactRequest`].
//! - [`ExitAcc`]: the exit (hand-down) key sets of `HandDowns::Keys`, de-duplicated incrementally (sort + dedup when the
//!   buffered words double) instead of keeping every term's code until the end of a chunk.
//! - Both SPILL to files when a [`SpillConfig`] is given and their in-memory part exceeds its budget: requests are
//!   appended to `fact-requests.bin`, exit keys are written as sorted runs and k-way merged at the end into one key-set
//!   file per child sector (`exits-<mask>.keyset`, schema `rustred.campaign.keyset.v1`). Memory bound per store: about
//!   twice the budget (vector growth). The RESULT (sets, counts, digests, requests) never depends on the budget, the
//!   thread count or the batching; only the intermediate files do.

use std::collections::{BTreeMap, BinaryHeap};
use std::fs::File;
use std::io::{self, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::facts::RequestReason;
use super::keyset::{self, DigestStream, KEYSET_SCHEMA, KeyCodec, KeySet, MAX_WORDS};

/// Where and when a closure run spills its large stores to files (re-audit F5, F9).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpillConfig {
    /// Parent directory: the run creates a fresh sub-directory `run-<n>` (first free n) and writes only there; files are
    /// never deleted except its own temporary run files.
    pub dir: PathBuf,
    /// In-memory budget PER STORE (fact requests; exit keys) in bytes; the resident part of each store stays below
    /// about twice this value.
    pub budget_bytes: u64,
}

/// A spilled exit key set: the key-set file of one child sector.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExitFile {
    pub path: PathBuf,
    pub count: u64,
    /// The key-set digest (`sha256:..`, the canonical stream of [`KeySet::digest`]).
    pub digest: String,
}

/// Create the run's own spill directory `<parent>/run-<n>` (first n that does not exist yet).
pub(super) fn make_run_dir(parent: &Path) -> io::Result<PathBuf> {
    std::fs::create_dir_all(parent)?;
    for n in 0u32.. {
        let d = parent.join(format!("run-{n}"));
        match std::fs::create_dir(&d) {
            Ok(()) => return Ok(d),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    unreachable!("u32 run directories exhausted")
}

// ------------------------------------------------------------------------------------------------ fact requests

const TERM_BITS: u32 = 28;
const TERM_MASK: u32 = (1 << TERM_BITS) - 1;
/// Term field of a regularity / witness-lift request (no term).
pub(super) const TERM_NONE: u32 = TERM_MASK;
const REASON_SHIFT: u32 = TERM_BITS;
const IMAGE_BIT: u32 = 1 << 30;
const EXPLICIT_BIT: u32 = 1 << 31;

fn reason_code(r: RequestReason) -> u32 {
    match r {
        RequestReason::PositiveNumerator => 0,
        RequestReason::NotLower => 1,
        RequestReason::Regularity => 2,
        RequestReason::WitnessLift => 3,
    }
}

fn reason_of(c: u32) -> RequestReason {
    match c & 3 {
        0 => RequestReason::PositiveNumerator,
        1 => RequestReason::NotLower,
        2 => RequestReason::Regularity,
        _ => RequestReason::WitnessLift,
    }
}

/// One request as read back from a [`ReqStore`].
#[derive(Clone, Copy, Debug)]
pub(super) struct ReqItem<'a> {
    pub key: u32,
    pub pos: u32,
    pub term: Option<u32>,
    pub reason: RequestReason,
    /// The rule is applied at the credited key's image (the key index is the credited key).
    pub image: bool,
    pub rhs: Option<&'a [i32]>,
}

/// Fact requests in production order (key-index order per round; deterministic, thread independent).
#[derive(Clone, Debug)]
pub(super) struct ReqStore {
    codec: KeyCodec,
    recs: Vec<[u32; 3]>,
    packed: Vec<u64>,
    explicit: Vec<i32>,
    /// Spilled records, in order: (file, records); the run's own file is the last one when it spilled.
    segments: Vec<(PathBuf, u64)>,
    /// The run's own spill file and the budget (bytes).
    own: Option<(PathBuf, usize)>,
    total: u64,
}

impl ReqStore {
    pub(super) fn new(codec: KeyCodec) -> ReqStore {
        ReqStore {
            codec,
            recs: vec![],
            packed: vec![],
            explicit: vec![],
            segments: vec![],
            own: None,
            total: 0,
        }
    }
    /// Spill to `path` (appended) whenever the in-memory part exceeds `budget` bytes.
    pub(super) fn set_spill(&mut self, path: PathBuf, budget: usize) {
        self.own = Some((path, budget.max(1 << 8)));
    }
    pub(super) fn is_spilled(&self) -> bool {
        !self.segments.is_empty()
    }
    /// Bytes held in memory (records + packed / explicit term keys).
    pub(super) fn memory_bytes(&self) -> usize {
        self.recs.capacity() * 12 + self.packed.capacity() * 8 + self.explicit.capacity() * 4
    }
    /// Append one request. `term` must be below 2^28 - 1 (else `Err`, a capacity error of the caller).
    pub(super) fn push(
        &mut self,
        key: u32,
        pos: u32,
        term: Option<usize>,
        reason: RequestReason,
        image: bool,
        rhs: Option<&[i32]>,
    ) -> Result<(), String> {
        let t = match term {
            None => TERM_NONE,
            Some(t) if t < TERM_NONE as usize => t as u32,
            Some(t) => return Err(format!("term index {t} above the request store's 2^28 - 2")),
        };
        let mut flags = reason_code(reason) << REASON_SHIFT;
        if image {
            flags |= IMAGE_BIT;
        }
        if t != TERM_NONE {
            let rhs = rhs.ok_or("coefficient request without its term key")?;
            let w = self.codec.words();
            let mut code = [0u64; MAX_WORDS];
            if self.codec.encode(rhs, &mut code) {
                self.packed.extend_from_slice(&code[..w]);
            } else {
                flags |= EXPLICIT_BIT;
                self.explicit.extend_from_slice(rhs);
            }
        }
        self.recs.push([key, pos, t | flags]);
        self.total += 1;
        Ok(())
    }
    /// Move every request of `o` (an unspilled local store) to the end of `self`; spill if over budget.
    pub(super) fn append(&mut self, o: &mut ReqStore) -> io::Result<()> {
        debug_assert!(o.segments.is_empty());
        self.recs.append(&mut o.recs);
        self.packed.append(&mut o.packed);
        self.explicit.append(&mut o.explicit);
        self.total += o.total;
        o.total = 0;
        self.maybe_spill()
    }
    fn maybe_spill(&mut self) -> io::Result<()> {
        let Some((path, budget)) = &self.own else {
            return Ok(());
        };
        if self.memory_bytes() <= *budget || self.recs.is_empty() {
            return Ok(());
        }
        let path = path.clone();
        let f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        let mut wr = BufWriter::with_capacity(1 << 20, f);
        let w = self.codec.words();
        let n = self.codec.arity();
        let (mut ip, mut ie) = (0usize, 0usize);
        for r in &self.recs {
            for x in r {
                wr.write_all(&x.to_le_bytes())?;
            }
            if r[2] & TERM_MASK != TERM_NONE {
                if r[2] & EXPLICIT_BIT != 0 {
                    for v in &self.explicit[ie..ie + n] {
                        wr.write_all(&v.to_le_bytes())?;
                    }
                    ie += n;
                } else {
                    for v in &self.packed[ip..ip + w] {
                        wr.write_all(&v.to_le_bytes())?;
                    }
                    ip += w;
                }
            }
        }
        wr.flush()?;
        let k = self.recs.len() as u64;
        match self.segments.last_mut() {
            Some((p, c)) if *p == path => *c += k,
            _ => self.segments.push((path, k)),
        }
        self.recs = vec![];
        self.packed = vec![];
        self.explicit = vec![];
        Ok(())
    }
    /// Visit every request in production order (spilled segments first, then memory).
    pub(super) fn for_each(&self, mut f: impl FnMut(ReqItem)) -> io::Result<()> {
        let w = self.codec.words();
        let n = self.codec.arity();
        let mut key = vec![0i32; n];
        for (path, count) in &self.segments {
            let mut rd = BufReader::with_capacity(1 << 20, File::open(path)?);
            let mut b4 = [0u8; 4];
            let mut b8 = [0u8; 8];
            let mut code = [0u64; MAX_WORDS];
            for _ in 0..*count {
                let mut r = [0u32; 3];
                for x in &mut r {
                    rd.read_exact(&mut b4)?;
                    *x = u32::from_le_bytes(b4);
                }
                let has = r[2] & TERM_MASK != TERM_NONE;
                if has {
                    if r[2] & EXPLICIT_BIT != 0 {
                        for v in key.iter_mut() {
                            rd.read_exact(&mut b4)?;
                            *v = i32::from_le_bytes(b4);
                        }
                    } else {
                        for c in code.iter_mut().take(w) {
                            rd.read_exact(&mut b8)?;
                            *c = u64::from_le_bytes(b8);
                        }
                        self.codec.decode(&code[..w], &mut key);
                    }
                }
                f(item(r, has.then_some(key.as_slice())));
            }
        }
        let (mut ip, mut ie) = (0usize, 0usize);
        for r in &self.recs {
            let has = r[2] & TERM_MASK != TERM_NONE;
            if has {
                if r[2] & EXPLICIT_BIT != 0 {
                    key.copy_from_slice(&self.explicit[ie..ie + n]);
                    ie += n;
                } else {
                    self.codec.decode(&self.packed[ip..ip + w], &mut key);
                    ip += w;
                }
            }
            f(item(*r, has.then_some(key.as_slice())));
        }
        Ok(())
    }
}

fn item(r: [u32; 3], rhs: Option<&[i32]>) -> ReqItem<'_> {
    let t = r[2] & TERM_MASK;
    ReqItem {
        key: r[0],
        pos: r[1],
        term: (t != TERM_NONE).then_some(t),
        reason: reason_of(r[2] >> REASON_SHIFT),
        image: r[2] & IMAGE_BIT != 0,
        rhs,
    }
}

// ------------------------------------------------------------------------------------------------ exit key sets

/// Words buffered before the first in-memory compaction (sort + dedup); later compactions at twice the kept size.
const COMPACT_BASE_WORDS: usize = 1 << 22;

#[derive(Clone, Debug)]
struct Run {
    path: PathBuf,
    /// A key-set file (skip its header line) rather than a raw run.
    keyset_file: bool,
    /// Created by this run (deleted after the merge).
    owned: bool,
}

/// Exit key sets per child sector (dropped-line mask), de-duplicated incrementally, optionally spilled.
#[derive(Debug)]
pub(super) struct ExitAcc {
    codec: KeyCodec,
    mem: BTreeMap<u64, Vec<u64>>,
    words: usize,
    compact_at: usize,
    runs: BTreeMap<u64, Vec<Run>>,
    /// (run directory, budget in words)
    spill: Option<(PathBuf, usize)>,
    seq: u64,
    peak_words: usize,
}

impl ExitAcc {
    pub(super) fn new(codec: KeyCodec, spill: Option<(PathBuf, usize)>) -> ExitAcc {
        let spill = spill.map(|(d, b)| (d, (b / 8).max(1 << 6)));
        let compact_at = match &spill {
            Some((_, b)) => (*b).min(COMPACT_BASE_WORDS),
            None => COMPACT_BASE_WORDS,
        };
        ExitAcc {
            codec,
            mem: BTreeMap::new(),
            words: 0,
            compact_at,
            runs: BTreeMap::new(),
            spill,
            seq: 0,
            peak_words: 0,
        }
    }
    /// Seed with a previous run's exit keys (recheck): in-memory sets and key-set files (read, never deleted).
    pub(super) fn seed(
        &mut self,
        sets: &BTreeMap<u64, KeySet>,
        files: &BTreeMap<u64, ExitFile>,
    ) -> io::Result<()> {
        for (&m, k) in sets {
            self.add(m, k.raw_codes())?;
        }
        for (&m, f) in files {
            if self.spill.is_none() {
                return Err(io::Error::other(
                    "recheck of a closure with spilled exit key sets needs a spill directory",
                ));
            }
            self.runs.entry(m).or_default().push(Run {
                path: f.path.clone(),
                keyset_file: true,
                owned: false,
            });
        }
        Ok(())
    }
    /// Add sorted or unsorted codes of child `m`.
    pub(super) fn add(&mut self, m: u64, codes: &[u64]) -> io::Result<()> {
        if codes.is_empty() {
            return Ok(());
        }
        self.mem.entry(m).or_default().extend_from_slice(codes);
        self.words += codes.len();
        self.peak_words = self.peak_words.max(self.words);
        if self.words > self.compact_at {
            self.compact()?;
        }
        Ok(())
    }
    fn compact(&mut self) -> io::Result<()> {
        let w = self.codec.words();
        let mut words = 0;
        for v in self.mem.values_mut() {
            keyset::sort_dedup(w, v);
            words += v.len();
        }
        self.words = words;
        if let Some((_, budget)) = &self.spill
            && 2 * words > *budget
        {
            self.write_runs()?;
        }
        self.compact_at = match &self.spill {
            Some((_, b)) => (2 * self.words).max(COMPACT_BASE_WORDS.min(*b)).min(*b),
            None => (2 * self.words).max(COMPACT_BASE_WORDS),
        };
        Ok(())
    }
    fn write_runs(&mut self) -> io::Result<()> {
        let Some((dir, _)) = &self.spill else {
            return Ok(());
        };
        let dir = dir.clone();
        let w = self.codec.words();
        for (m, v) in std::mem::take(&mut self.mem) {
            if v.is_empty() {
                continue;
            }
            let mut v = v;
            keyset::sort_dedup(w, &mut v);
            let path = dir.join(format!("exits-{m:016x}.run{}", self.seq));
            self.seq += 1;
            let mut wr = BufWriter::with_capacity(1 << 20, File::create(&path)?);
            for x in &v {
                wr.write_all(&x.to_le_bytes())?;
            }
            wr.flush()?;
            self.runs.entry(m).or_default().push(Run {
                path,
                keyset_file: false,
                owned: true,
            });
        }
        self.words = 0;
        Ok(())
    }
    /// Peak buffered words (statistics).
    pub(super) fn peak_bytes(&self) -> usize {
        self.peak_words * 8
    }
    /// Final sets: in memory when nothing was spilled, else one key-set file per child (every child, then).
    pub(super) fn finish(mut self) -> io::Result<(BTreeMap<u64, KeySet>, BTreeMap<u64, ExitFile>)> {
        let w = self.codec.words();
        if self.runs.is_empty() {
            let sets = std::mem::take(&mut self.mem)
                .into_iter()
                .map(|(m, v)| (m, KeySet::from_codes(self.codec, v)))
                .collect();
            return Ok((sets, BTreeMap::new()));
        }
        self.write_runs()?;
        let dir = self
            .spill
            .as_ref()
            .expect("runs need a spill dir")
            .0
            .clone();
        let mut files = BTreeMap::new();
        for (m, runs) in std::mem::take(&mut self.runs) {
            let body = dir.join(format!("exits-{m:016x}.body"));
            let (count, digest) = merge_runs(&self.codec, &runs, &body)?;
            let path = dir.join(format!("exits-{m:016x}.keyset"));
            {
                let mut out = BufWriter::with_capacity(1 << 20, File::create(&path)?);
                let header = serde_json::json!({
                    "schema": KEYSET_SCHEMA,
                    "arity": self.codec.arity(),
                    "bits": self.codec.bits(),
                    "words": w,
                    "count": count,
                    "digest": digest,
                });
                writeln!(out, "{header}")?;
                let mut rd = BufReader::with_capacity(1 << 20, File::open(&body)?);
                io::copy(&mut rd, &mut out)?;
                out.flush()?;
            }
            std::fs::remove_file(&body)?;
            for r in runs.iter().filter(|r| r.owned) {
                std::fs::remove_file(&r.path)?;
            }
            files.insert(
                m,
                ExitFile {
                    path,
                    count,
                    digest,
                },
            );
        }
        Ok((BTreeMap::new(), files))
    }
}

/// Sequential reader of sorted codes (a raw run or the body of a key-set file).
struct RunReader {
    rd: BufReader<File>,
    w: usize,
    left: Option<u64>,
}

impl RunReader {
    fn open(r: &Run, codec: &KeyCodec) -> io::Result<RunReader> {
        let mut rd = BufReader::with_capacity(1 << 18, File::open(&r.path)?);
        let mut left = None;
        if r.keyset_file {
            let mut line = String::new();
            rd.read_line(&mut line)?;
            let h: serde_json::Value = serde_json::from_str(line.trim_end())
                .map_err(|e| io::Error::other(format!("{}: header {e}", r.path.display())))?;
            if h["schema"] != KEYSET_SCHEMA
                || h["arity"].as_u64() != Some(codec.arity() as u64)
                || h["bits"].as_u64() != Some(codec.bits() as u64)
            {
                return Err(io::Error::other(format!(
                    "{}: key-set file of another codec",
                    r.path.display()
                )));
            }
            left = h["count"].as_u64();
        }
        Ok(RunReader {
            rd,
            w: codec.words(),
            left,
        })
    }
    fn next(&mut self) -> io::Result<Option<[u64; MAX_WORDS]>> {
        if self.left == Some(0) {
            return Ok(None);
        }
        let mut c = [0u64; MAX_WORDS];
        let mut b8 = [0u8; 8];
        for (i, x) in c.iter_mut().take(self.w).enumerate() {
            match self.rd.read_exact(&mut b8) {
                Ok(()) => *x = u64::from_le_bytes(b8),
                Err(e)
                    if e.kind() == io::ErrorKind::UnexpectedEof
                        && i == 0
                        && self.left.is_none() =>
                {
                    return Ok(None);
                }
                Err(e) => return Err(e),
            }
        }
        if let Some(l) = &mut self.left {
            *l -= 1;
        }
        Ok(Some(c))
    }
}

/// k-way merge (with de-duplication) of sorted runs into `body` (raw codes); returns (count, canonical digest).
fn merge_runs(codec: &KeyCodec, runs: &[Run], body: &Path) -> io::Result<(u64, String)> {
    let w = codec.words();
    let mut readers = runs
        .iter()
        .map(|r| RunReader::open(r, codec))
        .collect::<io::Result<Vec<_>>>()?;
    let mut heap = BinaryHeap::new();
    for (i, r) in readers.iter_mut().enumerate() {
        if let Some(c) = r.next()? {
            heap.push(std::cmp::Reverse((c, i)));
        }
    }
    let mut out = BufWriter::with_capacity(1 << 20, File::create(body)?);
    let mut dig = DigestStream::new(codec.arity());
    let mut key = vec![0i32; codec.arity()];
    let mut last: Option<[u64; MAX_WORDS]> = None;
    let mut count = 0u64;
    while let Some(std::cmp::Reverse((c, i))) = heap.pop() {
        if last != Some(c) {
            for x in &c[..w] {
                out.write_all(&x.to_le_bytes())?;
            }
            codec.decode(&c[..w], &mut key);
            dig.key(&key);
            count += 1;
            last = Some(c);
        }
        if let Some(n) = readers[i].next()? {
            heap.push(std::cmp::Reverse((n, i)));
        }
    }
    out.flush()?;
    Ok((count, dig.finish()))
}
