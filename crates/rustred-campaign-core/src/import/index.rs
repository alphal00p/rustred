//! On-disk INDEX of a table file (lane import3, item 2): the table is read ONCE, in one streaming pass, and what the
//! import needs from it is written to a small directory that later loads read instead of the table.
//!
//! The pass (a reader thread, two hashing threads, the parse on the calling thread; at most 4 threads, the bytes in
//! flight bounded by [`IN_FLIGHT_BYTES`] except for one line larger than that): the frame from the header line, every
//! line classified and parsed by the shared [`LineScanner`] (same refusals as a direct scan), the whole-file sha256
//! and every rule line's `record_sha256` computed WHILE reading.
//!
//! LONG LINES (lane bigline): a line longer than the reader limits' `whole_line_max` (spec `reader`) is never held
//! whole: the reader thread sends it as PIECES of at most [`PIECE_BYTES`] (after a first piece of `whole_line_max`), the
//! hashing threads hash the pieces, and the calling thread STREAMS it ([`crate::record::stream::stream_line`]) into its
//! compact form (the rhs value replaced by `[]`: exactly the payload written for the line) and its rhs keys. The index
//! files are byte-identical whichever path a line took.
//!
//! Index directory (`rustred.campaign.table-index.v1`):
//! - `domains.jsonl`: per non-blank table line an entry `{"k": kind, "l": line, "o": offset, "b": bytes, "s": sha256}`
//!   (sha256 for rule lines), followed for header / chart / rule / sym lines and the content kinds
//!   ([`super::lines::CONTENT_KINDS`]: residual, terminal ledger, completeness and provenance markers; version 2) by the
//!   PAYLOAD: the original line, a rule line with its `rhs` value replaced by `[]` (every other field byte-identical); a
//!   `sym` record is entered as `rule` (its body = its image);
//! - `bodies.jsonl`: per rule line `{"o": ordinal, "s": record_sha256, "rhs": ["I(..)", ..]}` (the rhs KEYS, never a
//!   coefficient);
//! - `index.json` (written LAST: its presence marks a complete index): schema, [`INDEX_VERSION`], canonical path,
//!   size, mtime (ns), a sampled digest ([`sample_digest`]: sha256 over [`SAMPLE_BLOCKS`] blocks of [`SAMPLE_BYTES`]
//!   spread over the file), the whole-file sha256, line kinds, rule-record count.
//!
//! REUSE (a cache, simple versioning as the user rules: no fingerprint machinery): an index is reused iff its version
//! equals [`INDEX_VERSION`] and the file's canonical path, size, mtime and sampled digest equal the recorded ones; the
//! whole-file sha256 is then TAKEN FROM THE INDEX (stated in the load report), not recomputed. Anything else rebuilds it
//! (written to a fresh directory, renamed into place). Without a cache directory the index lives in a private
//! temporary directory removed when the [`TableIndex`] is dropped (one pass over the table all the same).

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Instant, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::error::{ImportError, io_err};
use super::error::TableRefusal;
use super::lines::{CONTENT_KINDS, Fed, LineScanner, RawRecord, RawTable, line_body};
use crate::closure::sha256::Sha256;
use crate::record::stream::stream_line;
use crate::record::{ReadLimits, RhsMode, read_until_limited};

/// Version of the index format and of what it promises. Bump on ANY change of the files or of their meaning.
/// 3 (lane bigline): `index.json` records the sha256 of `domains.jsonl` and `bodies.jsonl`, verified on every read.
pub const INDEX_VERSION: u32 = 3;
/// Schema word of `index.json`.
pub const INDEX_SCHEMA: &str = "rustred.campaign.table-index.v1";
/// Blocks read for the sampled digest.
pub const SAMPLE_BLOCKS: u64 = 64;
/// Bytes per sampled block.
pub const SAMPLE_BYTES: usize = 1 << 16;
/// Bytes of table lines in flight between the reader and its consumers (one larger line is admitted alone).
pub const IN_FLIGHT_BYTES: usize = 256 << 20;
/// Size of the pieces of a long line after its first piece (I/O granularity only; what a record may hold is the
/// spec's per-record budget).
pub const PIECE_BYTES: u64 = 4 << 20;

/// Callback of [`TableIndex::scan_bodies`]: (ordinal, record_sha256, rhs keys).
pub type BodyVisitor<'a> = dyn FnMut(usize, &str, Vec<String>) -> Result<(), ImportError> + 'a;

/// `index.json`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexMeta {
    pub schema: String,
    pub version: u32,
    /// Canonical path of the indexed table.
    pub path: String,
    pub size: u64,
    pub mtime_ns: u128,
    /// [`sample_digest`] of the file when it was indexed.
    pub sample_sha256: String,
    /// sha256 of the whole file, computed during the indexing pass.
    pub file_sha256: String,
    pub lines: usize,
    pub rule_records: usize,
    pub kinds: BTreeMap<String, usize>,
    /// sha256 of `domains.jsonl` / `bodies.jsonl` as written by the build: every read of an index file is checked
    /// against them (an index file of another build, e.g. of another table in a shared directory, is refused).
    pub domains_sha256: String,
    pub bodies_sha256: String,
    /// Random id of the build that wrote the three files.
    pub build_id: String,
}

/// How an index was obtained (for the load report; NOT identifying).
#[derive(Clone, Debug, Serialize)]
pub struct IndexOrigin {
    /// `reused` | `built` | `built_temporary`.
    pub how: &'static str,
    pub dir: String,
    /// Seconds to validate (reuse) or to build (one pass over the table).
    pub seconds: f64,
    pub table_bytes: u64,
}

/// An open, validated index of one table file.
#[derive(Debug)]
pub struct TableIndex {
    pub dir: PathBuf,
    pub meta: IndexMeta,
    /// The indexed table as the caller named it (error messages).
    pub table: PathBuf,
    /// The reader limits the index is read under (an index line longer than the budget is refused).
    pub limits: ReadLimits,
    /// A temporary index: the directory this process created (removed with the index); None = persistent.
    owned: Option<OwnedDir>,
}

/// A directory THIS process created by an exclusive mkdir ([`unique_dir`]): the ONLY kind of directory the import
/// ever removes recursively (rule L18: never a pre-existing directory). Removed with its content when dropped, unless
/// released (renamed into place, or kept).
#[derive(Debug)]
pub struct OwnedDir {
    path: PathBuf,
    armed: bool,
}

impl OwnedDir {
    pub fn path(&self) -> &Path {
        &self.path
    }
    /// Give up ownership: the directory is no longer removed by this guard (it was renamed away, or must stay).
    pub fn release(mut self) -> PathBuf {
        self.armed = false;
        std::mem::take(&mut self.path)
    }
}

impl Drop for OwnedDir {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

/// The files of an index directory (the only names [`remove_index_files`] removes).
pub const INDEX_FILES: &[&str] = &["index.json", "domains.jsonl", "bodies.jsonl"];

/// Remove a STALE persistent index directory that this process did not create: its known files by name, then the
/// directory with `rmdir` (never recursively): a directory holding anything else is left in place (Err).
pub fn remove_index_files(dir: &Path) -> std::io::Result<()> {
    for f in INDEX_FILES {
        match std::fs::remove_file(dir.join(f)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    match std::fs::remove_dir(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

#[derive(Serialize, Deserialize)]
struct Entry {
    k: String,
    l: usize,
    o: u64,
    b: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    s: Option<String>,
}

#[derive(Deserialize)]
struct BodyLine {
    o: usize,
    s: String,
    rhs: Vec<String>,
}

fn mtime_ns(md: &std::fs::Metadata) -> u128 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos())
}

/// sha256 over the file size and [`SAMPLE_BLOCKS`] blocks of [`SAMPLE_BYTES`] at evenly spread offsets (the first
/// block at 0, the last ending at the end of the file), each prefixed by its offset.
pub fn sample_digest(path: &Path) -> Result<String, ImportError> {
    let mut f = std::fs::File::open(path).map_err(|e| io_err(path, e))?;
    let size = f.metadata().map_err(|e| io_err(path, e))?.len();
    let mut h = Sha256::new();
    h.update(format!("size {size}\n").as_bytes());
    let blk = SAMPLE_BYTES as u64;
    let mut buf = vec![0u8; SAMPLE_BYTES];
    for i in 0..SAMPLE_BLOCKS {
        let off = if size <= blk {
            0
        } else {
            i * (size - blk) / (SAMPLE_BLOCKS - 1)
        };
        f.seek(SeekFrom::Start(off)).map_err(|e| io_err(path, e))?;
        let mut got = 0usize;
        while got < SAMPLE_BYTES {
            let n = f.read(&mut buf[got..]).map_err(|e| io_err(path, e))?;
            if n == 0 {
                break;
            }
            got += n;
        }
        h.update(format!("@{off}\n").as_bytes());
        h.update(&buf[..got]);
        if size <= blk {
            break;
        }
    }
    Ok(h.finish_hex())
}

fn canonical(path: &Path) -> Result<PathBuf, ImportError> {
    std::fs::canonicalize(path).map_err(|e| io_err(path, e))
}

/// The index directory of `path` under `cache` (name = first 16 hex of the sha256 of the canonical path + file name).
pub fn index_dir_for(cache: &Path, path: &Path) -> Result<PathBuf, ImportError> {
    let c = canonical(path)?;
    let mut h = Sha256::new();
    h.update(c.display().to_string().as_bytes());
    let hex = h.finish_hex();
    let name = c
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    Ok(cache.join(format!("{}-{name}", &hex[..16])))
}

/// The existing index in `dir` if it is complete and describes `path` as it is now (version, canonical path, size,
/// mtime, sampled digest); `None` otherwise.
pub fn validate(dir: &Path, path: &Path) -> Result<Option<IndexMeta>, ImportError> {
    let Ok(t) = std::fs::read_to_string(dir.join("index.json")) else {
        return Ok(None);
    };
    let Ok(m) = serde_json::from_str::<IndexMeta>(&t) else {
        return Ok(None);
    };
    let md = std::fs::metadata(path).map_err(|e| io_err(path, e))?;
    if m.schema != INDEX_SCHEMA
        || m.version != INDEX_VERSION
        || m.path != canonical(path)?.display().to_string()
        || m.size != md.len()
        || m.mtime_ns != mtime_ns(&md)
        || !dir.join("domains.jsonl").is_file()
        || !dir.join("bodies.jsonl").is_file()
    {
        return Ok(None);
    }
    if m.sample_sha256 != sample_digest(path)? {
        return Ok(None);
    }
    Ok(Some(m))
}

static TEMP_SEQ: AtomicUsize = AtomicUsize::new(0);

/// 64 random bits from the OS (`/dev/urandom`; else the standard library's randomly seeded hasher keys).
fn os_nonce() -> u64 {
    let mut b = [0u8; 8];
    if std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut b))
        .is_ok()
    {
        return u64::from_le_bytes(b);
    }
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_usize(TEMP_SEQ.fetch_add(1, Ordering::Relaxed));
    h.finish()
}

/// The process's start (clock ticks since boot, `/proc/self/stat` field 22; 0 where unavailable).
fn process_start() -> u64 {
    std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|s| {
            let rest = s.rsplit_once(')')?.1.to_string();
            rest.split_whitespace().nth(19)?.parse().ok()
        })
        .unwrap_or(0)
}

/// A NEW private directory `<parent>/<prefix>-<pid>-<start>-<nonce>` created by an EXCLUSIVE mkdir (retried with a
/// fresh OS nonce if the name exists): unique even for processes with the same pid (private pid namespaces, where
/// every run is pid 1) sharing one temporary directory, whatever TMPDIR is (lane bigline; defect found by lane
/// repairflow2). Returned as an [`OwnedDir`] guard: removed when dropped unless released.
pub fn unique_dir(parent: &Path, prefix: &str) -> Result<OwnedDir, ImportError> {
    std::fs::create_dir_all(parent).map_err(|e| io_err(parent, e))?;
    let (pid, start) = (std::process::id(), process_start());
    loop {
        let d = parent.join(format!("{prefix}-{pid}-{start}-{:016x}", os_nonce()));
        match std::fs::create_dir(&d) {
            Ok(()) => {
                return Ok(OwnedDir {
                    path: d,
                    armed: true,
                });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(io_err(&d, e)),
        }
    }
}

/// Open the index of `path`: reuse a valid one under `cache`, else build it there (or, without `cache`, in a private
/// temporary directory under `std::env::temp_dir()`, removed with the returned [`TableIndex`]). No per-record limit
/// ([`open_or_build_with`]).
pub fn open_or_build(
    source: &str,
    path: &Path,
    cache: Option<&Path>,
) -> Result<(TableIndex, IndexOrigin), ImportError> {
    open_or_build_with(source, path, cache, ReadLimits::unlimited())
}

/// [`open_or_build`] under per-record memory limits (lines above `limits.whole_line_max` streamed; a record part
/// that must be held and exceeds `limits.record_budget` refuses the table, typed).
pub fn open_or_build_with(
    source: &str,
    path: &Path,
    cache: Option<&Path>,
    limits: ReadLimits,
) -> Result<(TableIndex, IndexOrigin), ImportError> {
    open_inner(source, path, cache, limits, false)
}

/// `rebuild`: never reuse (a persistent index that failed a check is replaced).
fn open_inner(
    source: &str,
    path: &Path,
    cache: Option<&Path>,
    limits: ReadLimits,
    rebuild: bool,
) -> Result<(TableIndex, IndexOrigin), ImportError> {
    let t0 = Instant::now();
    let table_bytes = std::fs::metadata(path).map_err(|e| io_err(path, e))?.len();
    let origin = |how: &'static str, dir: &Path| IndexOrigin {
        how,
        dir: dir.display().to_string(),
        seconds: t0.elapsed().as_secs_f64(),
        table_bytes,
    };
    let Some(c) = cache else {
        // a fresh private directory (exclusive mkdir), never one of another process
        let own = unique_dir(&std::env::temp_dir(), "rr-import-index")?;
        let meta = build(source, path, own.path(), limits)?;
        let ix = TableIndex {
            dir: own.path().to_path_buf(),
            meta,
            table: path.to_path_buf(),
            limits,
            owned: Some(own),
        };
        ix.check_table()?;
        let o = origin("built_temporary", &ix.dir);
        return Ok((ix, o));
    };
    std::fs::create_dir_all(c).map_err(|e| io_err(c, e))?;
    let dir = index_dir_for(c, path)?;
    if !rebuild && let Some(meta) = validate(&dir, path)? {
        let ix = TableIndex {
            dir: dir.clone(),
            meta,
            table: path.to_path_buf(),
            limits,
            owned: None,
        };
        return Ok((ix, origin("reused", &dir)));
    }
    // a private build directory beside the index (exclusive mkdir): concurrent builders never share one
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    let work = unique_dir(c, &format!("{name}.building"))?;
    let mut meta = build(source, path, work.path(), limits)?;
    match validate(&dir, path)? {
        // another builder put a valid index in place meanwhile: use it (ours goes with its guard)
        Some(m) if !rebuild => meta = m,
        _ => {
            // replace a stale index: its known files only, never a directory with anything else in it
            remove_index_files(&dir).map_err(|e| io_err(&dir, e))?;
            match std::fs::rename(work.path(), &dir) {
                Ok(()) => {
                    work.release();
                }
                Err(e) => match validate(&dir, path)? {
                    Some(m) => meta = m,
                    None => return Err(io_err(&dir, e)),
                },
            }
        }
    }
    let ix = TableIndex {
        dir: dir.clone(),
        meta,
        table: path.to_path_buf(),
        limits,
        owned: None,
    };
    ix.check_table()?;
    Ok((ix, origin("built", &dir)))
}

/// A RawTable read from a verified index: [`TableIndex::scan_domains`] (every record), then the spot check
/// ([`TableIndex::spot_check`], `spot` records re-hashed from the table bytes). An index that fails the spot check
/// or any of its read checks is refused and rebuilt ONCE (a persistent one in place); a failure of the rebuilt index
/// is an error.
pub fn open_scan_checked(
    source: &str,
    path: &Path,
    cache: Option<&Path>,
    limits: ReadLimits,
    spot: usize,
) -> Result<(TableIndex, IndexOrigin, RawTable, Option<String>), ImportError> {
    let attempt = |rebuild: bool| -> Result<(TableIndex, IndexOrigin, RawTable), ImportError> {
        let (ix, o) = open_inner(source, path, cache, limits, rebuild)?;
        let mut recs = vec![];
        let mut raw = ix.scan_domains(source, &|_, _| true, &mut |r| {
            recs.push(r);
            Ok(())
        })?;
        raw.records = recs;
        ix.spot_check(&raw.records, spot)?;
        Ok((ix, o, raw))
    };
    match attempt(false) {
        Ok((ix, o, raw)) => Ok((ix, o, raw, None)),
        Err(e @ (ImportError::Digest { .. } | ImportError::Source { .. })) => {
            let why = format!("index refused and rebuilt: {e}");
            let (ix, o, raw) = attempt(true)?;
            Ok((ix, o, raw, Some(why)))
        }
        Err(e) => Err(e),
    }
}

/// Byte budget of the lines in flight.
struct Budget {
    used: Mutex<usize>,
    cv: Condvar,
}

impl Budget {
    fn acquire(&self, n: usize) {
        let mut u = self.used.lock().expect("budget");
        while *u > 0 && *u + n > IN_FLIGHT_BYTES {
            u = self.cv.wait(u).expect("budget");
        }
        *u += n;
    }
    fn release(&self, n: usize) {
        let mut u = self.used.lock().expect("budget");
        *u -= n;
        self.cv.notify_all();
    }
}

/// A piece of a table line shared by the reader's consumers; its bytes return to the budget when the last one drops
/// it. A line up to the whole-line limit is ONE piece (`last`); a longer line is a sequence ending with a `last` piece.
struct Line {
    buf: Vec<u8>,
    /// The piece ends its line (it holds the newline, or the input ended).
    last: bool,
    budget: Arc<Budget>,
}

impl Drop for Line {
    fn drop(&mut self) {
        self.budget.release(self.buf.len());
    }
}

/// The reader thread: every line (with its newline) to the three consumers, in file order; a line longer than
/// `whole_line_max` as pieces.
fn reader(
    path: PathBuf,
    budget: Arc<Budget>,
    outs: [SyncSender<Arc<Line>>; 3],
    whole_line_max: u64,
) -> Result<u64, std::io::Error> {
    let f = std::fs::File::open(&path)?;
    let mut rd = BufReader::with_capacity(4 << 20, f);
    let mut total = 0u64;
    let mut in_line = false;
    loop {
        let mut buf = Vec::new();
        let limit = if in_line {
            PIECE_BYTES
        } else {
            whole_line_max.saturating_add(1)
        };
        let (n, last) = read_until_limited(&mut rd, &mut buf, limit)?;
        if n == 0 && !in_line {
            return Ok(total);
        }
        total += n as u64;
        in_line = !last;
        budget.acquire(buf.len());
        let line = Arc::new(Line {
            buf,
            last,
            budget: budget.clone(),
        });
        for o in &outs {
            if o.send(line.clone()).is_err() {
                // a consumer stopped (an error is reported by the calling thread)
                return Ok(total);
            }
        }
    }
}

/// The pieces of ONE long line as a [`BufRead`] (the calling thread's view while it streams the line).
struct PieceRead<'a> {
    cur: Option<Arc<Line>>,
    pos: usize,
    rx: &'a Receiver<Arc<Line>>,
    total: u64,
}

impl std::io::Read for PieceRead<'_> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        let b = self.fill_buf()?;
        let n = b.len().min(out.len());
        out[..n].copy_from_slice(&b[..n]);
        self.consume(n);
        Ok(n)
    }
}

impl BufRead for PieceRead<'_> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        loop {
            match &self.cur {
                Some(c) if self.pos < c.buf.len() => break,
                Some(c) if c.last => return Ok(&[]),
                _ => {}
            }
            let next = self.rx.recv().map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "index reader stopped inside a line",
                )
            })?;
            self.cur = Some(next);
            self.pos = 0;
        }
        let c = self.cur.as_ref().expect("piece");
        Ok(&c.buf[self.pos..])
    }
    fn consume(&mut self, n: usize) {
        self.pos += n;
        self.total += n as u64;
    }
}

/// Stream `path` once and write the index files into `work` (index.json last).
fn build(
    source: &str,
    path: &Path,
    work: &Path,
    limits: ReadLimits,
) -> Result<IndexMeta, ImportError> {
    let md0 = std::fs::metadata(path).map_err(|e| io_err(path, e))?;
    let sample = sample_digest(path)?;
    let budget = Arc::new(Budget {
        used: Mutex::new(0),
        cv: Condvar::new(),
    });
    let (tx_file, rx_file) = sync_channel::<Arc<Line>>(256);
    let (tx_line, rx_line) = sync_channel::<Arc<Line>>(256);
    let (tx_main, rx_main) = sync_channel::<Arc<Line>>(256);
    let (tx_sha, rx_sha) = sync_channel::<String>(1 << 16);
    let p = path.to_path_buf();
    let b = budget.clone();
    let wl = limits.whole_line_max;
    let rd = std::thread::spawn(move || reader(p, b, [tx_file, tx_line, tx_main], wl));
    let fh = std::thread::spawn(move || {
        let mut h = Sha256::new();
        for l in rx_file {
            h.update(&l.buf);
        }
        h.finish_hex()
    });
    let lh = std::thread::spawn(move || {
        // per line (one piece, or the pieces of a long line): the sha256 of the line without its newline
        let mut h: Option<(Sha256, bool)> = None;
        for l in rx_line {
            let data: &[u8] = if l.last {
                line_body(&l.buf)
            } else {
                &l.buf
            };
            let (hh, blank) = h.get_or_insert_with(|| (Sha256::new(), true));
            *blank &= data.iter().all(|c| c.is_ascii_whitespace());
            hh.update(data);
            if l.last {
                let (hh, blank) = h.take().expect("line hash");
                if !blank && tx_sha.send(hh.finish_hex()).is_err() {
                    return;
                }
            }
        }
    });
    let res = consume(source, path, work, rx_main, rx_sha, limits);
    let total = rd
        .join()
        .map_err(|_| ImportError::Core("index reader thread panicked".into()))?
        .map_err(|e| io_err(path, e))?;
    let file_sha256 = fh
        .join()
        .map_err(|_| ImportError::Core("index hash thread panicked".into()))?;
    lh.join()
        .map_err(|_| ImportError::Core("index line-hash thread panicked".into()))?;
    let c = res?;
    let md1 = std::fs::metadata(path).map_err(|e| io_err(path, e))?;
    if md1.len() != md0.len() || mtime_ns(&md1) != mtime_ns(&md0) || total != md0.len() {
        return Err(ImportError::Source {
            source: source.to_string(),
            msg: format!("{} changed while it was indexed", path.display()),
        });
    }
    let meta = IndexMeta {
        schema: INDEX_SCHEMA.into(),
        version: INDEX_VERSION,
        path: canonical(path)?.display().to_string(),
        size: md0.len(),
        mtime_ns: mtime_ns(&md0),
        sample_sha256: sample,
        file_sha256,
        lines: c.lines,
        rule_records: c.rules,
        kinds: c.kinds,
        domains_sha256: c.domains_sha256,
        bodies_sha256: c.bodies_sha256,
        build_id: format!("{:016x}{:016x}", os_nonce(), os_nonce()),
    };
    let ip = work.join("index.json");
    std::fs::write(&ip, serde_json::to_string_pretty(&meta).expect("meta"))
        .map_err(|e| io_err(&ip, e))?;
    Ok(meta)
}

/// The calling thread's part of the pass: parse every line (rhs keys only), write `domains.jsonl` and `bodies.jsonl`.
fn consume(
    source: &str,
    path: &Path,
    work: &Path,
    rx: Receiver<Arc<Line>>,
    rx_sha: Receiver<String>,
    limits: ReadLimits,
) -> Result<Consumed, ImportError> {
    let dp = work.join("domains.jsonl");
    let bp = work.join("bodies.jsonl");
    let mut dw = HashWriter {
        w: BufWriter::with_capacity(
            1 << 20,
            std::fs::File::create(&dp).map_err(|e| io_err(&dp, e))?,
        ),
        h: Sha256::new(),
    };
    let mut bw = HashWriter {
        w: BufWriter::with_capacity(
            4 << 20,
            std::fs::File::create(&bp).map_err(|e| io_err(&bp, e))?,
        ),
        h: Sha256::new(),
    };
    let mut sc = LineScanner::new(source, path, RhsMode::KeysOnly, false).with_limits(limits);
    let (mut offset, mut line_no, mut rules) = (0u64, 0usize, 0usize);
    while let Ok(l) = rx.recv() {
        line_no += 1;
        let start = offset;
        // a long line arrives in pieces: streamed into its compact form (the payload) and its rhs keys
        let streamed = if l.last {
            None
        } else {
            let mut pr = PieceRead {
                cur: Some(l.clone()),
                pos: 0,
                rx: &rx,
                total: 0,
            };
            let sl = stream_line(&mut pr, &sc.stream_options(), &mut |_| {}, None)
                .map_err(|e| sc.record_error(line_no, e))?;
            offset += pr.total;
            if sl.blank {
                continue;
            }
            Some(sl)
        };
        let body: &[u8] = match &streamed {
            Some(sl) => &sl.compact,
            None => {
                offset += l.buf.len() as u64;
                line_body(&l.buf)
            }
        };
        if body.iter().all(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let sha = rx_sha
            .recv()
            .map_err(|_| ImportError::Core("index line-hash thread stopped".into()))?;
        let bytes = streamed.as_ref().map_or(body.len() as u64, |sl| sl.bytes);
        let entry = |k: &str, s: Option<String>| -> String {
            let mut t = serde_json::to_string(&Entry {
                k: k.to_string(),
                l: line_no,
                o: start,
                b: bytes,
                s,
            })
            .expect("entry");
            t.push('\n');
            t
        };
        let fed = match &streamed {
            Some(sl) => sc.feed_streamed(sl, line_no, start, Some(sha.clone()), &|_, _| true)?,
            None => sc.feed(body, line_no, start, bytes, Some(sha.clone()), &|_, _| true)?,
        };
        // the rhs value's span in `body`: a streamed line's compact form already holds `[]` there
        let replaced = streamed.as_ref().is_some_and(|sl| !sl.exact());
        match fed {
            Fed::Blank | Fed::RuleNotKept => {}
            f @ (Fed::Header | Fed::Chart) => {
                let k = if matches!(f, Fed::Header) {
                    "header"
                } else {
                    "chart"
                };
                dw.put(&dp, entry(k, None).as_bytes())?;
                dw.put(&dp, body)?;
                dw.put(&dp, b"\n")?;
            }
            Fed::Other(kind) => {
                dw.put(&dp, entry(&kind, None).as_bytes())?;
                if CONTENT_KINDS.contains(&kind.as_str()) {
                    dw.put(&dp, body)?;
                    dw.put(&dp, b"\n")?;
                }
            }
            Fed::Rule(r) => {
                rules += 1;
                dw.put(&dp, entry("rule", Some(sha.clone())).as_bytes())?;
                match r.rhs_span.filter(|_| !replaced) {
                    Some((a, b)) => {
                        dw.put(&dp, &body[..a])?;
                        dw.put(&dp, b"[]")?;
                        dw.put(&dp, &body[b..])?;
                    }
                    None => dw.put(&dp, body)?,
                }
                dw.put(&dp, b"\n")?;
                let keys = r.rec.rhs_keys.as_deref().unwrap_or(&[]);
                let mut t = format!("{{\"o\":{},\"s\":\"{}\",\"rhs\":", r.ordinal, r.sha256);
                t.push_str(&serde_json::to_string(keys).expect("keys"));
                t.push_str("}\n");
                bw.put(&bp, t.as_bytes())?;
            }
        }
    }
    dw.w.flush().map_err(|e| io_err(&dp, e))?;
    bw.w.flush().map_err(|e| io_err(&bp, e))?;
    let raw = sc.finish(String::new(), offset)?;
    Ok(Consumed {
        lines: line_no,
        kinds: raw.kinds,
        rules,
        domains_sha256: dw.h.finish_hex(),
        bodies_sha256: bw.h.finish_hex(),
    })
}

/// What the parse thread of the index pass produced.
struct Consumed {
    lines: usize,
    kinds: BTreeMap<String, usize>,
    rules: usize,
    domains_sha256: String,
    bodies_sha256: String,
}

/// A buffered index-file writer that hashes what it writes.
struct HashWriter {
    w: BufWriter<std::fs::File>,
    h: Sha256,
}

impl HashWriter {
    fn put(&mut self, p: &Path, b: &[u8]) -> Result<(), ImportError> {
        self.h.update(b);
        self.w.write_all(b).map_err(|e| io_err(p, e))
    }
}

impl TableIndex {
    /// A temporary index (its directory, created by this process, goes with it).
    pub fn is_temporary(&self) -> bool {
        self.owned.is_some()
    }

    /// The TABLE FILE is still the one the index describes: size, mtime and the sampled digest against the meta
    /// (temporary and persistent indexes alike; the meta's whole-file sha256 was computed from the table bytes by
    /// the build that wrote it).
    pub fn check_table(&self) -> Result<(), ImportError> {
        let md = std::fs::metadata(&self.table).map_err(|e| io_err(&self.table, e))?;
        let bad = |what: &str| ImportError::Source {
            source: "index".into(),
            msg: format!(
                "index {} does not describe {} ({what} differs)",
                self.dir.display(),
                self.table.display()
            ),
        };
        if md.len() != self.meta.size {
            return Err(bad("size"));
        }
        if mtime_ns(&md) != self.meta.mtime_ns {
            return Err(bad("mtime"));
        }
        if canonical(&self.table)?.display().to_string() != self.meta.path {
            return Err(bad("path"));
        }
        if sample_digest(&self.table)? != self.meta.sample_sha256 {
            return Err(bad("sampled digest"));
        }
        Ok(())
    }

    /// SPOT CHECK against the table bytes: `n` records spread over the file (the first and the last included) are
    /// re-read at their offset and re-hashed (streamed, never held) against their `record_sha256`, with their line
    /// boundaries (a newline or the start before, a newline or the end after). A mismatch is a Digest error (the
    /// caller refuses the index and rebuilds it).
    pub fn spot_check(&self, records: &[RawRecord], n: usize) -> Result<(), ImportError> {
        if records.is_empty() || n == 0 {
            return Ok(());
        }
        let m = records.len();
        let mut pick: Vec<usize> = if n >= m {
            (0..m).collect()
        } else {
            (0..n).map(|i| i * (m - 1) / (n - 1).max(1)).collect()
        };
        pick.push(m - 1);
        pick.sort_unstable();
        pick.dedup();
        let p = &self.table;
        let mut f = std::fs::File::open(p).map_err(|e| io_err(p, e))?;
        let size = self.meta.size;
        let mut buf = vec![0u8; 1 << 20];
        for &k in &pick {
            let r = &records[k];
            let bad = |what: String| ImportError::Digest {
                path: format!("{}@{}", p.display(), r.offset),
                expected: r.sha256.clone(),
                got: what,
            };
            if r.offset + r.bytes > size {
                return Err(bad("beyond the end of the table".into()));
            }
            let start = r.offset.saturating_sub(1);
            f.seek(SeekFrom::Start(start)).map_err(|e| io_err(p, e))?;
            let mut h = Sha256::new();
            let mut left = r.bytes + (r.offset - start);
            let mut first = r.offset > 0;
            while left > 0 {
                let want = left.min(buf.len() as u64) as usize;
                f.read_exact(&mut buf[..want]).map_err(|e| io_err(p, e))?;
                let mut chunk = &buf[..want];
                if first {
                    if chunk[0] != b'\n' {
                        return Err(bad("no line start at the offset".into()));
                    }
                    chunk = &chunk[1..];
                    first = false;
                }
                h.update(chunk);
                left -= want as u64;
            }
            if r.offset + r.bytes < size {
                let mut nl = [0u8; 1];
                f.read_exact(&mut nl).map_err(|e| io_err(p, e))?;
                if nl[0] != b'\n' {
                    return Err(bad("no line end after the record".into()));
                }
            }
            let got = h.finish_hex();
            if got != r.sha256 {
                return Err(bad(got));
            }
        }
        Ok(())
    }

    /// One line of an index file, at most the per-record budget (plus the newline): a longer one (an index built
    /// under a larger budget) refuses, typed. `line`: the table line it describes (0 = not known).
    fn bounded_line(
        &self,
        rd: &mut BufReader<std::fs::File>,
        buf: &mut Vec<u8>,
        file: &Path,
        source: &str,
        line: usize,
        h: &mut Sha256,
    ) -> Result<usize, ImportError> {
        let limit = self.limits.record_budget.saturating_add(1);
        let (n, complete) = read_until_limited(rd, buf, limit).map_err(|e| io_err(file, e))?;
        h.update(buf);
        if !complete {
            // measure the line without keeping it
            let mut more = n as u64;
            loop {
                let mut sink = Vec::new();
                let (m, done) = read_until_limited(rd, &mut sink, 1 << 20).map_err(|e| io_err(file, e))?;
                more += m as u64;
                if done {
                    break;
                }
            }
            return Err(ImportError::Refused {
                source: source.to_string(),
                path: self.table.display().to_string(),
                line,
                reason: Box::new(TableRefusal::RecordTooLarge {
                    record: None,
                    what: format!("an index line of {}", file.display()),
                    bytes: more,
                    budget: self.limits.record_budget,
                }),
            });
        }
        Ok(n)
    }

    /// The end of a read of an index file: its sha256 must be the one its build recorded (the file is this index's,
    /// not another build's, e.g. of another table in a shared directory).
    fn verify_file(&self, file: &Path, h: Sha256, expected: &str) -> Result<(), ImportError> {
        let got = h.finish_hex();
        if got != expected {
            return Err(ImportError::Digest {
                path: file.display().to_string(),
                expected: expected.to_string(),
                got,
            });
        }
        Ok(())
    }

    /// The table as the index describes it, every kept record WITHOUT its body (`rec.rhs_keys` = None), in file
    /// order: the same [`RawTable`] / [`RawRecord`] fields a direct scan gives (`file_sha256` from the index).
    pub fn scan_domains(
        &self,
        source: &str,
        keep: &dyn Fn(usize, Option<i64>) -> bool,
        visit: &mut dyn FnMut(RawRecord) -> Result<(), ImportError>,
    ) -> Result<RawTable, ImportError> {
        let dp = self.dir.join("domains.jsonl");
        let f = std::fs::File::open(&dp).map_err(|e| io_err(&dp, e))?;
        let mut rd = BufReader::with_capacity(1 << 20, f);
        let mut sc =
            LineScanner::new(source, &self.table, RhsMode::Skip, false).with_limits(self.limits);
        let bad = |m: String| ImportError::Source {
            source: source.to_string(),
            msg: format!("index {}: {m}", self.dir.display()),
        };
        self.check_table()?;
        let mut eb: Vec<u8> = vec![];
        let mut pb: Vec<u8> = vec![];
        let mut fh = Sha256::new();
        let mut rule_entries = 0usize;
        loop {
            eb.clear();
            if self.bounded_line(&mut rd, &mut eb, &dp, source, 0, &mut fh)? == 0 {
                break;
            }
            let e: Entry =
                serde_json::from_slice(line_body(&eb)).map_err(|x| bad(format!("entry: {x}")))?;
            match e.k.as_str() {
                "header" | "chart" | "rule" => {
                    pb.clear();
                    if self.bounded_line(&mut rd, &mut pb, &dp, source, e.l, &mut fh)? == 0 {
                        return Err(bad("payload missing".into()));
                    }
                    if e.k == "rule" {
                        rule_entries += 1;
                    }
                    let fed = sc.feed(line_body(&pb), e.l, e.o, e.b, e.s.clone(), keep)?;
                    let ok = match (e.k.as_str(), fed) {
                        ("header", Fed::Header)
                        | ("chart", Fed::Chart)
                        | ("rule", Fed::RuleNotKept) => true,
                        ("rule", Fed::Rule(r)) => {
                            visit(*r)?;
                            true
                        }
                        _ => false,
                    };
                    if !ok {
                        return Err(bad(format!("line {}: payload is not a {}", e.l, e.k)));
                    }
                }
                k if CONTENT_KINDS.contains(&k) => {
                    pb.clear();
                    if self.bounded_line(&mut rd, &mut pb, &dp, source, e.l, &mut fh)? == 0 {
                        return Err(bad("payload missing".into()));
                    }
                    match sc.feed(line_body(&pb), e.l, e.o, e.b, None, keep)? {
                        Fed::Other(k2) if k2 == k => {}
                        _ => return Err(bad(format!("line {}: payload is not a {k}", e.l))),
                    }
                }
                k => sc.count_kind(e.l, k)?,
            }
        }
        self.verify_file(&dp, fh, &self.meta.domains_sha256)?;
        if rule_entries != self.meta.rule_records {
            return Err(bad(format!(
                "{rule_entries} rule entries, the index records {}",
                self.meta.rule_records
            )));
        }
        sc.finish(self.meta.file_sha256.clone(), self.meta.size)
    }

    /// Every record body (rhs keys) whose ordinal `want` accepts, in file order: `visit(ordinal, record_sha256, keys)`.
    pub fn scan_bodies(
        &self,
        want: &dyn Fn(usize) -> bool,
        visit: &mut BodyVisitor,
    ) -> Result<(), ImportError> {
        let bp = self.dir.join("bodies.jsonl");
        let f = std::fs::File::open(&bp).map_err(|e| io_err(&bp, e))?;
        let mut rd = BufReader::with_capacity(4 << 20, f);
        let mut buf: Vec<u8> = vec![];
        let mut fh = Sha256::new();
        let mut lines = 0usize;
        loop {
            buf.clear();
            if self.bounded_line(&mut rd, &mut buf, &bp, "index", 0, &mut fh)? == 0 {
                self.verify_file(&bp, fh, &self.meta.bodies_sha256)?;
                if lines != self.meta.rule_records {
                    return Err(ImportError::Source {
                        source: "index".into(),
                        msg: format!(
                            "index {}: {lines} bodies, the index records {} rule records",
                            self.dir.display(),
                            self.meta.rule_records
                        ),
                    });
                }
                return Ok(());
            }
            lines += 1;
            // the ordinal first (cheap) so that unwanted bodies are not parsed
            let o = buf
                .get(5..)
                .and_then(|t| {
                    let end = t.iter().position(|c| !c.is_ascii_digit())?;
                    std::str::from_utf8(&t[..end]).ok()?.parse::<usize>().ok()
                })
                .ok_or_else(|| ImportError::Core(format!("index {}: body line", bp.display())))?;
            if !want(o) {
                continue;
            }
            let b: BodyLine = serde_json::from_slice(line_body(&buf))
                .map_err(|e| ImportError::Core(format!("index {}: {e}", bp.display())))?;
            debug_assert_eq!(b.o, o);
            visit(b.o, &b.s, b.rhs)?;
        }
    }
}
