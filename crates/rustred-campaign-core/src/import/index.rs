//! On-disk INDEX of a table file (lane import3, item 2): the table is read ONCE, in one streaming pass, and what the
//! import needs from it is written to a small directory that later loads read instead of the table.
//!
//! The pass (a reader thread, two hashing threads, the parse on the calling thread; at most 4 threads, the bytes in
//! flight bounded by [`IN_FLIGHT_BYTES`] except for one line larger than that): the frame from the header line, every
//! line classified and parsed by the shared [`LineScanner`] (same refusals as a direct scan), the whole-file sha256
//! and every rule line's `record_sha256` computed WHILE reading.
//!
//! Index directory (`rustred.campaign.table-index.v1`):
//! - `domains.jsonl`: per non-blank table line an entry `{"k": kind, "l": line, "o": offset, "b": bytes, "s": sha256}`
//!   (sha256 for rule lines), followed for header / chart / rule lines by the PAYLOAD: the original line, a rule line
//!   with its `rhs` value replaced by `[]` (every other field byte-identical);
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
use super::lines::{Fed, LineScanner, RawRecord, RawTable, line_body};
use crate::closure::sha256::Sha256;
use crate::record::RhsMode;

/// Version of the index format and of what it promises. Bump on ANY change of the files or of their meaning.
pub const INDEX_VERSION: u32 = 1;
/// Schema word of `index.json`.
pub const INDEX_SCHEMA: &str = "rustred.campaign.table-index.v1";
/// Blocks read for the sampled digest.
pub const SAMPLE_BLOCKS: u64 = 64;
/// Bytes per sampled block.
pub const SAMPLE_BYTES: usize = 1 << 16;
/// Bytes of table lines in flight between the reader and its consumers (one larger line is admitted alone).
pub const IN_FLIGHT_BYTES: usize = 256 << 20;

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
    temporary: bool,
}

impl Drop for TableIndex {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
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

/// Open the index of `path`: reuse a valid one under `cache`, else build it there (or, without `cache`, in a private
/// temporary directory under `std::env::temp_dir()`, removed with the returned [`TableIndex`]).
pub fn open_or_build(
    source: &str,
    path: &Path,
    cache: Option<&Path>,
) -> Result<(TableIndex, IndexOrigin), ImportError> {
    let t0 = Instant::now();
    let table_bytes = std::fs::metadata(path).map_err(|e| io_err(path, e))?.len();
    let (dir, temporary) = match cache {
        Some(c) => {
            std::fs::create_dir_all(c).map_err(|e| io_err(c, e))?;
            (index_dir_for(c, path)?, false)
        }
        None => (
            std::env::temp_dir().join(format!(
                "rr-import-index-{}-{}",
                std::process::id(),
                TEMP_SEQ.fetch_add(1, Ordering::Relaxed)
            )),
            true,
        ),
    };
    if !temporary && let Some(meta) = validate(&dir, path)? {
        return Ok((
            TableIndex {
                dir: dir.clone(),
                meta,
                table: path.to_path_buf(),
                temporary: false,
            },
            IndexOrigin {
                how: "reused",
                dir: dir.display().to_string(),
                seconds: t0.elapsed().as_secs_f64(),
                table_bytes,
            },
        ));
    }
    let work = if temporary {
        dir.clone()
    } else {
        let mut w = dir.clone().into_os_string();
        w.push(format!(".building-{}", std::process::id()));
        PathBuf::from(w)
    };
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| io_err(&work, e))?;
    let meta = match build(source, path, &work) {
        Ok(m) => m,
        Err(e) => {
            let _ = std::fs::remove_dir_all(&work);
            return Err(e);
        }
    };
    if !temporary {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::rename(&work, &dir).map_err(|e| io_err(&dir, e))?;
    }
    Ok((
        TableIndex {
            dir: dir.clone(),
            meta,
            table: path.to_path_buf(),
            temporary,
        },
        IndexOrigin {
            how: if temporary {
                "built_temporary"
            } else {
                "built"
            },
            dir: dir.display().to_string(),
            seconds: t0.elapsed().as_secs_f64(),
            table_bytes,
        },
    ))
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

/// One table line shared by the reader's consumers; its bytes return to the budget when the last one drops it.
struct Line {
    buf: Vec<u8>,
    budget: Arc<Budget>,
}

impl Drop for Line {
    fn drop(&mut self) {
        self.budget.release(self.buf.len());
    }
}

/// The reader thread: every line (with its newline) to the three consumers, in file order.
fn reader(
    path: PathBuf,
    budget: Arc<Budget>,
    outs: [SyncSender<Arc<Line>>; 3],
) -> Result<u64, std::io::Error> {
    let f = std::fs::File::open(&path)?;
    let mut rd = BufReader::with_capacity(4 << 20, f);
    let mut total = 0u64;
    loop {
        let mut buf = Vec::new();
        let n = rd.read_until(b'\n', &mut buf)?;
        if n == 0 {
            return Ok(total);
        }
        total += n as u64;
        budget.acquire(buf.len());
        let line = Arc::new(Line {
            buf,
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

/// Stream `path` once and write the index files into `work` (index.json last).
fn build(source: &str, path: &Path, work: &Path) -> Result<IndexMeta, ImportError> {
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
    let rd = std::thread::spawn(move || reader(p, b, [tx_file, tx_line, tx_main]));
    let fh = std::thread::spawn(move || {
        let mut h = Sha256::new();
        for l in rx_file {
            h.update(&l.buf);
        }
        h.finish_hex()
    });
    let lh = std::thread::spawn(move || {
        for l in rx_line {
            let body = line_body(&l.buf);
            if body.iter().all(|c| c.is_ascii_whitespace()) {
                continue;
            }
            let mut h = Sha256::new();
            h.update(body);
            if tx_sha.send(h.finish_hex()).is_err() {
                return;
            }
        }
    });
    let res = consume(source, path, work, rx_main, rx_sha);
    let total = rd
        .join()
        .map_err(|_| ImportError::Core("index reader thread panicked".into()))?
        .map_err(|e| io_err(path, e))?;
    let file_sha256 = fh
        .join()
        .map_err(|_| ImportError::Core("index hash thread panicked".into()))?;
    lh.join()
        .map_err(|_| ImportError::Core("index line-hash thread panicked".into()))?;
    let (lines, raw) = res?;
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
        lines,
        rule_records: raw.1,
        kinds: raw.0,
    };
    let ip = work.join("index.json");
    std::fs::write(&ip, serde_json::to_string_pretty(&meta).expect("meta"))
        .map_err(|e| io_err(&ip, e))?;
    Ok(meta)
}

/// The calling thread's part of the pass: parse every line (rhs keys only), write `domains.jsonl` and `bodies.jsonl`.
#[allow(clippy::type_complexity)]
fn consume(
    source: &str,
    path: &Path,
    work: &Path,
    rx: Receiver<Arc<Line>>,
    rx_sha: Receiver<String>,
) -> Result<(usize, (BTreeMap<String, usize>, usize)), ImportError> {
    let dp = work.join("domains.jsonl");
    let bp = work.join("bodies.jsonl");
    let mut dw = BufWriter::with_capacity(
        1 << 20,
        std::fs::File::create(&dp).map_err(|e| io_err(&dp, e))?,
    );
    let mut bw = BufWriter::with_capacity(
        4 << 20,
        std::fs::File::create(&bp).map_err(|e| io_err(&bp, e))?,
    );
    let mut sc = LineScanner::new(source, path, RhsMode::KeysOnly, false);
    let (mut offset, mut line_no, mut rules) = (0u64, 0usize, 0usize);
    let wr = |w: &mut BufWriter<std::fs::File>, p: &Path, b: &[u8]| -> Result<(), ImportError> {
        w.write_all(b).map_err(|e| io_err(p, e))
    };
    for l in rx {
        line_no += 1;
        let start = offset;
        offset += l.buf.len() as u64;
        let body = line_body(&l.buf);
        if body.iter().all(|c| c.is_ascii_whitespace()) {
            continue;
        }
        let sha = rx_sha
            .recv()
            .map_err(|_| ImportError::Core("index line-hash thread stopped".into()))?;
        let bytes = body.len() as u64;
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
        match sc.feed(body, line_no, start, bytes, Some(sha.clone()), &|_, _| true)? {
            Fed::Blank | Fed::RuleNotKept => {}
            f @ (Fed::Header | Fed::Chart) => {
                let k = if matches!(f, Fed::Header) {
                    "header"
                } else {
                    "chart"
                };
                wr(&mut dw, &dp, entry(k, None).as_bytes())?;
                wr(&mut dw, &dp, body)?;
                wr(&mut dw, &dp, b"\n")?;
            }
            Fed::Other(kind) => wr(&mut dw, &dp, entry(&kind, None).as_bytes())?,
            Fed::Rule(r) => {
                rules += 1;
                wr(&mut dw, &dp, entry("rule", Some(sha.clone())).as_bytes())?;
                match r.rhs_span {
                    Some((a, b)) => {
                        wr(&mut dw, &dp, &body[..a])?;
                        wr(&mut dw, &dp, b"[]")?;
                        wr(&mut dw, &dp, &body[b..])?;
                    }
                    None => wr(&mut dw, &dp, body)?,
                }
                wr(&mut dw, &dp, b"\n")?;
                let keys = r.rec.rhs_keys.as_deref().unwrap_or(&[]);
                let mut t = format!("{{\"o\":{},\"s\":\"{}\",\"rhs\":", r.ordinal, r.sha256);
                t.push_str(&serde_json::to_string(keys).expect("keys"));
                t.push_str("}\n");
                wr(&mut bw, &bp, t.as_bytes())?;
            }
        }
    }
    dw.flush().map_err(|e| io_err(&dp, e))?;
    bw.flush().map_err(|e| io_err(&bp, e))?;
    let raw = sc.finish(String::new(), offset)?;
    Ok((line_no, (raw.kinds, rules)))
}

impl TableIndex {
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
        let mut sc = LineScanner::new(source, &self.table, RhsMode::Skip, false);
        let bad = |m: String| ImportError::Source {
            source: source.to_string(),
            msg: format!("index {}: {m}", self.dir.display()),
        };
        let mut eb: Vec<u8> = vec![];
        let mut pb: Vec<u8> = vec![];
        loop {
            eb.clear();
            if rd.read_until(b'\n', &mut eb).map_err(|e| io_err(&dp, e))? == 0 {
                break;
            }
            let e: Entry =
                serde_json::from_slice(line_body(&eb)).map_err(|x| bad(format!("entry: {x}")))?;
            match e.k.as_str() {
                "header" | "chart" | "rule" => {
                    pb.clear();
                    if rd.read_until(b'\n', &mut pb).map_err(|x| io_err(&dp, x))? == 0 {
                        return Err(bad("payload missing".into()));
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
                k => sc.count_kind(e.l, k)?,
            }
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
        loop {
            buf.clear();
            if rd.read_until(b'\n', &mut buf).map_err(|e| io_err(&bp, e))? == 0 {
                return Ok(());
            }
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
