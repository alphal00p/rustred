//! The STREAMED walk of one parent (lane arrivals; interfaces/arrivals.md section 8). Same arrival sets and the same
//! file bytes as `walk` + `aggregate` (the in-memory route, kept as the verifier's independent route); memory bounded by
//! budgets instead of the closure size:
//! - COMPACT ROWS: keys one byte per slot in one sorted array, served rows as (key index, table, record) u32s, the
//!   applied key only where it differs from the row's key; the exact closure on key indices (one flag per key);
//! - hand-down decisions and the route transport FUSED per record group: no hand-down term is kept;
//! - per-edge accumulators keep the first sender as (served row, term, decision text id); its term key is
//!   re-instantiated from the imported set when the line is written;
//! - past `map_budget` accumulator entries everything is SPILLED to disk as sorted runs, k-way merged per edge at the end;
//! - parsed records are freed past a byte budget (`RecCache::with_budget`);
//! - each edge's file is written as a stream (`format::write_stream`), its lines formed in parallel blocks.
//!
//! Nothing here names a node, a count or a frame (L8).

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;

use super::*;

// ------------------------------------------------------------------------------------------------------------------
// Compact rows
// ------------------------------------------------------------------------------------------------------------------

/// The parent's closure rows (a check rows file), compact; same acceptance rules as `ParentRows::read`.
pub struct CompactRows {
    pub arity: usize,
    /// Every closure key, one byte per slot, sorted.
    keys: Vec<i8>,
    in_start: Vec<bool>,
    /// Per key: 0 served by a rule record, 1 terminal (declared), 2 open (uncovered / blocked).
    kind: Vec<u8>,
    /// Rule-served rows in key order: key index, table, record.
    s_key: Vec<u32>,
    s_table: Vec<u32>,
    s_record: Vec<u32>,
    /// The applied key of a served row where it differs from the row's key (credits, sym images).
    s_applied: HashMap<u32, Box<[i32]>>,
    pub terminal: u64,
    pub open: u64,
    /// sha256 of the rows file bytes (hex).
    pub sha256: String,
    pub path: String,
}

fn json_key(v: &Value, at: &dyn Fn() -> String, what: &str) -> Result<Vec<i32>, String> {
    serde_json::from_value(v.clone()).map_err(|e| format!("{}: {what}: {e}", at()))
}

/// One rows-file line as read (unknown fields skipped).
#[derive(serde::Deserialize)]
struct RowIn {
    key: Vec<i32>,
    #[serde(default)]
    in_start: Value,
    #[serde(default)]
    status: Value,
    #[serde(default)]
    server: Value,
    #[serde(default)]
    image: Value,
}

/// A parsed row: key bytes, in_start, (table, record) or u32::MAX, the applied key where it differs, 0 served /
/// 1 terminal / 2 open.
type ParsedRow = (CKey, bool, (u32, u32), Option<Box<[i32]>>, u8);

/// Parse one rows-file line (same acceptance rules as `ParentRows::read`).
fn parse_row(
    set: &ImportedSet,
    n: usize,
    l: &str,
    at: &dyn Fn() -> String,
) -> Result<ParsedRow, String> {
    let v: RowIn = serde_json::from_str(l).map_err(|e| format!("{}: {e}", at()))?;
    let key = v.key;
    if key.len() != n {
        return Err(format!(
            "{}: key of arity {} (the set's is {n})",
            at(),
            key.len()
        ));
    }
    let kb = ck(&key)?;
    let in_start = v.in_start == true;
    match v.status.as_str() {
        Some("served") => {}
        Some("terminal") => return Ok((kb, in_start, (u32::MAX, 0), None, 1)),
        Some("open") => return Ok((kb, in_start, (u32::MAX, 0), None, 2)),
        s => return Err(format!("{}: status {s:?}", at())),
    }
    let kind = v.server["kind"].as_str().unwrap_or("");
    let app: Option<Vec<i32>> = match kind {
        // a sym-served rule row carries its image too (import5): the record is applied there
        "rule" => match v.server.get("image") {
            Some(img) if !img.is_null() => Some(json_key(img, at, "image")?),
            _ => None,
        },
        "credit" => match v.server.get("image").or(Some(&v.image)) {
            Some(img) if !img.is_null() => Some(json_key(img, at, "image")?),
            _ => {
                return Err(format!(
                    "{}: a credit row without its image (the walk cannot apply the record): refused",
                    at()
                ));
            }
        },
        k => {
            return Err(format!(
                "{}: a key served by {k:?} (not a table rule): the arrival walk refuses (fail closed)",
                at()
            ));
        }
    };
    let sha = v.server["record_sha256"]
        .as_str()
        .ok_or_else(|| format!("{}: server without record_sha256", at()))?;
    let (t, r) = set
        .find_sha(sha)
        .ok_or_else(|| format!("{}: record {sha} is not in the parent's table set", at()))?;
    let applied = app.filter(|a| *a != key).map(Vec::into_boxed_slice);
    Ok((kb, in_start, (t as u32, r as u32), applied, 0))
}

/// Bytes of rows-file lines read before a batch is parsed (in parallel).
const ROWS_BATCH: usize = 256 << 20;

impl CompactRows {
    /// Read a rows file against the parent's imported set (records found by their sha256 in the set); lines parsed in
    /// parallel batches by `threads` workers (the file hash and the row order are those of a sequential read).
    pub fn read(set: &ImportedSet, path: &Path, threads: usize) -> Result<CompactRows, String> {
        use std::io::BufRead;
        let n = set.arity();
        let f = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut rd = BufReader::with_capacity(1 << 22, f);
        let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
        let mut keys: Vec<i8> = vec![];
        let mut in_start: Vec<bool> = vec![];
        let mut kinds: Vec<u8> = vec![];
        // per key in file order: (table, record), u32::MAX = not rule-served
        let mut srv: Vec<(u32, u32)> = vec![];
        let mut applied: HashMap<u32, Box<[i32]>> = HashMap::new();
        let (mut terminal, mut open) = (0u64, 0u64);
        let mut line_no = 0usize;
        let mut done = false;
        while !done {
            // a batch of (line number, line) in file order; the hash is updated line by line as `lines()` would see it
            let mut batch: Vec<(usize, String)> = vec![];
            let mut bytes = 0usize;
            while bytes < ROWS_BATCH {
                let mut s = String::new();
                let k = rd
                    .read_line(&mut s)
                    .map_err(|e| format!("{}: {e}", path.display()))?;
                if k == 0 {
                    done = true;
                    break;
                }
                if s.ends_with('\n') {
                    s.pop();
                    if s.ends_with('\r') {
                        s.pop();
                    }
                }
                line_no += 1;
                h.update(s.as_bytes());
                h.update(b"\n");
                bytes += k;
                if !s.trim().is_empty() {
                    batch.push((line_no, s));
                }
            }
            let chunk = batch.len().div_ceil(threads.max(1)).max(1);
            let parts: Vec<&[(usize, String)]> = batch.chunks(chunk).collect();
            let parsed = par_map(&parts, threads, |part| -> Result<Vec<ParsedRow>, String> {
                part.iter()
                    .map(|(i, l)| {
                        let at = || format!("{} line {i}", path.display());
                        parse_row(set, n, l, &at)
                    })
                    .collect()
            });
            for p in parsed {
                for (kb, s, sv, ap, kind) in p? {
                    let idx = u32::try_from(in_start.len())
                        .map_err(|_| format!("{}: too many rows", path.display()))?;
                    keys.extend_from_slice(&kb);
                    in_start.push(s);
                    kinds.push(kind);
                    srv.push(sv);
                    match kind {
                        1 => terminal += 1,
                        2 => open += 1,
                        _ => {}
                    }
                    if let Some(a) = ap {
                        applied.insert(idx, a);
                    }
                }
            }
        }
        let nk = in_start.len();
        let kat = |i: usize| &keys[i * n..(i + 1) * n];
        let sorted = (1..nk).all(|i| kat(i - 1) < kat(i));
        let mut perm: Vec<u32> = (0..nk as u32).collect();
        if !sorted {
            perm.sort_unstable_by(|&a, &b| kat(a as usize).cmp(kat(b as usize)));
        }
        for w in perm.windows(2) {
            if kat(w[0] as usize) == kat(w[1] as usize) {
                let k: Vec<i32> = kat(w[0] as usize).iter().map(|&x| i32::from(x)).collect();
                return Err(format!("{}: key {k:?} twice", path.display()));
            }
        }
        let mut out = CompactRows {
            arity: n,
            keys: Vec::with_capacity(nk * n),
            in_start: Vec::with_capacity(nk),
            kind: Vec::with_capacity(nk),
            s_key: vec![],
            s_table: vec![],
            s_record: vec![],
            s_applied: HashMap::new(),
            terminal,
            open,
            sha256: h.finish_hex(),
            path: path.display().to_string(),
        };
        for (pos, &o) in perm.iter().enumerate() {
            let o = o as usize;
            out.keys.extend_from_slice(kat(o));
            out.in_start.push(in_start[o]);
            out.kind.push(kinds[o]);
            let (t, r) = srv[o];
            if t != u32::MAX {
                let si = out.s_key.len() as u32;
                out.s_key.push(pos as u32);
                out.s_table.push(t);
                out.s_record.push(r);
                if let Some(a) = applied.remove(&(o as u32)) {
                    out.s_applied.insert(si, a);
                }
            }
        }
        Ok(out)
    }

    pub fn n_keys(&self) -> usize {
        self.in_start.len()
    }

    /// The row kind of key `ki`: 0 served by a rule record, 1 terminal, 2 open.
    pub fn kind(&self, ki: usize) -> u8 {
        self.kind[ki]
    }

    /// Whether key `ki` is a start key (`in_start`) of the rows.
    pub fn is_start(&self, ki: usize) -> bool {
        self.in_start[ki]
    }

    pub fn n_served(&self) -> usize {
        self.s_key.len()
    }

    fn key(&self, ki: usize) -> &[i8] {
        &self.keys[ki * self.arity..(ki + 1) * self.arity]
    }

    pub fn key_i32(&self, ki: usize) -> Vec<i32> {
        self.key(ki).iter().map(|&x| i32::from(x)).collect()
    }

    /// The index of a closure key (None: not a key of the rows).
    pub fn find(&self, k: &[i32]) -> Option<usize> {
        if k.len() != self.arity {
            return None;
        }
        let kb: Vec<i8> = k
            .iter()
            .map(|&x| i8::try_from(x).ok())
            .collect::<Option<_>>()?;
        let (mut lo, mut hi) = (0usize, self.n_keys());
        while lo < hi {
            let mid = (lo + hi) / 2;
            match self.key(mid).cmp(kb.as_slice()) {
                std::cmp::Ordering::Less => lo = mid + 1,
                std::cmp::Ordering::Greater => hi = mid,
                std::cmp::Ordering::Equal => return Some(mid),
            }
        }
        None
    }

    /// The served row of a key (None: terminal / open).
    fn served_of(&self, ki: usize) -> Option<usize> {
        self.s_key.binary_search(&(ki as u32)).ok()
    }

    pub fn served_key(&self, si: usize) -> usize {
        self.s_key[si] as usize
    }

    /// The key the record of served row `si` is applied at.
    pub fn applied(&self, si: usize) -> Vec<i32> {
        match self.s_applied.get(&(si as u32)) {
            Some(a) => a.to_vec(),
            None => self.key_i32(self.served_key(si)),
        }
    }

    /// (table, record) of served row `si`.
    pub fn rec(&self, si: usize) -> RecKey {
        (self.s_table[si] as usize, self.s_record[si] as usize)
    }

    /// Every record that serves a row.
    pub fn records(&self) -> BTreeSet<RecKey> {
        (0..self.n_served()).map(|si| self.rec(si)).collect()
    }

    pub fn id(&self) -> RowsId {
        RowsId {
            path: self.path.clone(),
            sha256: self.sha256.clone(),
            keys: self.n_keys(),
            served: self.n_served(),
        }
    }
}

// ------------------------------------------------------------------------------------------------------------------
// Accumulators, decision texts, spill runs
// ------------------------------------------------------------------------------------------------------------------

/// A sender in compact form: (served row, term index, decision text id), ordered by (row, term) (the order of `walk`'s
/// first senders: parent keys in key order, then terms).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct First {
    row: u32,
    j: u32,
    by: u32,
}

fn fmin(a: First, b: First) -> First {
    if (a.row, a.j) <= (b.row, b.j) { a } else { b }
}

/// Per child key: the support senders, the exact level, the first senders (same merge as `merge_acc`).
#[derive(Clone, Copy, Debug)]
struct SAcc {
    senders: u64,
    exact_senders: u64,
    first: First,
    first_exact: Option<First>,
    exact_level: bool,
    /// The chain depth of the key: min over senders of (the sender's closure depth + 1).
    depth: u8,
}

fn smerge(b: &mut SAcc, a: SAcc) {
    b.depth = b.depth.min(a.depth);
    b.senders += a.senders;
    b.first = fmin(a.first, b.first);
    b.exact_senders += a.exact_senders;
    b.exact_level |= a.exact_level;
    b.first_exact = match (a.first_exact, b.first_exact) {
        (Some(x), Some(y)) => Some(fmin(x, y)),
        (x, y) => x.or(y),
    };
}

/// The decision texts (`by`) seen in this process, by id.
static BY_TABLE: Mutex<Vec<String>> = Mutex::new(Vec::new());

thread_local! {
    static BY_IDS: std::cell::RefCell<HashMap<String, u32>> = std::cell::RefCell::new(HashMap::new());
}

fn by_id(s: &str) -> Result<u32, String> {
    if let Some(i) = BY_IDS.with(|c| c.borrow().get(s).copied()) {
        return Ok(i);
    }
    let i = {
        let mut t = BY_TABLE
            .lock()
            .map_err(|_| "decision texts lock".to_string())?;
        match t.iter().position(|x| x == s) {
            Some(i) => i,
            None => {
                t.push(s.to_string());
                t.len() - 1
            }
        }
    };
    let i = u32::try_from(i).map_err(|_| "too many decision texts".to_string())?;
    BY_IDS.with(|c| c.borrow_mut().insert(s.to_string(), i));
    Ok(i)
}

fn by_text(i: u32) -> Result<String, String> {
    let t = BY_TABLE
        .lock()
        .map_err(|_| "decision texts lock".to_string())?;
    t.get(i as usize)
        .cloned()
        .ok_or_else(|| format!("decision text {i} unknown"))
}

/// Run record: key length (u8), key bytes, senders u64, exact_senders u64, flags u8 (1 = exact level, 2 = first_exact),
/// first (row, term, text) 3 x u32, first_exact 3 x u32 (zeros when absent), depth u8; little endian.
fn put_run(w: &mut impl Write, k: &[i8], a: &SAcc) -> std::io::Result<()> {
    w.write_all(&[k.len() as u8])?;
    w.write_all(&k.iter().map(|&x| x as u8).collect::<Vec<u8>>())?;
    w.write_all(&a.senders.to_le_bytes())?;
    w.write_all(&a.exact_senders.to_le_bytes())?;
    w.write_all(&[(a.exact_level as u8) | ((a.first_exact.is_some() as u8) << 1)])?;
    let fe = a.first_exact.unwrap_or(First {
        row: 0,
        j: 0,
        by: 0,
    });
    for v in [a.first.row, a.first.j, a.first.by, fe.row, fe.j, fe.by] {
        w.write_all(&v.to_le_bytes())?;
    }
    w.write_all(&[a.depth])?;
    Ok(())
}

struct RunReader {
    r: BufReader<std::fs::File>,
    path: PathBuf,
}

impl RunReader {
    fn next(&mut self) -> Result<Option<(CKey, SAcc)>, String> {
        let err = |e: std::io::Error| format!("{}: {e}", self.path.display());
        let mut l = [0u8; 1];
        match self.r.read(&mut l) {
            Ok(0) => return Ok(None),
            Ok(_) => {}
            Err(e) => return Err(err(e)),
        }
        let mut kb = vec![0u8; l[0] as usize];
        self.r.read_exact(&mut kb).map_err(err)?;
        let mut b = [0u8; 8 + 8 + 1 + 24 + 1];
        self.r.read_exact(&mut b).map_err(err)?;
        let u64at = |i: usize| u64::from_le_bytes(b[i..i + 8].try_into().expect("8"));
        let u32at = |i: usize| u32::from_le_bytes(b[i..i + 4].try_into().expect("4"));
        let flags = b[16];
        let first = First {
            row: u32at(17),
            j: u32at(21),
            by: u32at(25),
        };
        let fe = First {
            row: u32at(29),
            j: u32at(33),
            by: u32at(37),
        };
        let k: CKey = kb.into_iter().map(|x| x as i8).collect();
        Ok(Some((
            k,
            SAcc {
                senders: u64at(0),
                exact_senders: u64at(8),
                first,
                first_exact: (flags & 2 != 0).then_some(fe),
                exact_level: flags & 1 != 0,
                depth: b[41],
            },
        )))
    }
}

/// The shared per-edge accumulators and their spill runs.
struct Accs {
    maps: Vec<HashMap<CKey, SAcc>>,
    entries: usize,
    runs: Vec<Vec<PathBuf>>,
    spills: u64,
}

/// Parameters of the streamed walk (budgets; the walk's own parameters stay in `WalkCfg`).
#[derive(Clone, Debug)]
pub struct StreamCfg {
    /// Accumulator entries (all edges) kept in memory before a spill to `spill_dir`.
    pub map_budget: usize,
    pub spill_dir: PathBuf,
    /// Keep the decided hand-down terms of this many closure keys (evenly spaced): the second-route sample.
    pub decisions_sample: usize,
    /// Collect the distinct routed hand-down term keys (diagnostic; memory grows with them).
    pub term_keys: bool,
    /// CHAIN DEPTH: per key of the rows (CompactRows index), the depth of a start key (0 = the node's own start box, d =
    /// arrived by d hand-down steps; 255 = not a start key). None = not tracked (every start key 0, no depth written).
    pub start_depth: Option<Vec<u8>>,
    /// Keys whose CLOSURE PATH is reported (`StreamResult::trace`): from the key back to a start key, each step the closure
    /// parent (served row, top term) that first reached it with a non-zero coefficient.
    pub trace: Vec<Vec<i32>>,
    /// Stop after the closure (no hand-down, no edge file emitted).
    pub closure_only: bool,
    /// Write the exact closure (every reached key with its chain depth and row kind) to this file (RRKEYS01 format; module
    /// fn `write_closure`) and its open keys to `<file>.uncovered.jsonl`.
    pub closure_out: Option<PathBuf>,
}

/// Keys of a record group decided per work item (the record is held only while a chunk is decided).
const CHUNK: usize = 20_000;

/// Entries a worker gathers before merging them into the shared accumulators.
const LOCAL_FLUSH: usize = 1 << 21;

fn merge_into(
    accs: &Mutex<Accs>,
    local: &mut [HashMap<CKey, SAcc>],
    cfg: &StreamCfg,
) -> Result<(), String> {
    let mut g = accs.lock().map_err(|_| "accumulator lock".to_string())?;
    for (e, m) in local.iter_mut().enumerate() {
        for (k, a) in m.drain() {
            match g.maps[e].entry(k) {
                std::collections::hash_map::Entry::Occupied(mut o) => smerge(o.get_mut(), a),
                std::collections::hash_map::Entry::Vacant(v) => {
                    v.insert(a);
                    g.entries += 1;
                }
            }
        }
    }
    if g.entries > cfg.map_budget {
        spill(&mut g, cfg)?;
    }
    Ok(())
}

/// The entries of an accumulator map sorted by key, with a memory peak of max(map, vector) instead of map + vector: the entries
/// are streamed unsorted to `tmp` (run record format), the map is freed, and they are read back into a vector of exact capacity.
fn sorted_entries(m: HashMap<CKey, SAcc>, tmp: &Path) -> Result<Vec<(CKey, SAcc)>, String> {
    let n = m.len();
    if n == 0 {
        return Ok(vec![]);
    }
    let err = |x: std::io::Error| format!("{}: {x}", tmp.display());
    {
        let f = std::fs::File::create(tmp).map_err(err)?;
        let mut w = BufWriter::with_capacity(1 << 20, f);
        for (k, a) in &m {
            put_run(&mut w, k, a).map_err(err)?;
        }
        w.flush().map_err(err)?;
    }
    drop(m);
    let f = std::fs::File::open(tmp).map_err(err)?;
    let mut rr = RunReader {
        r: BufReader::with_capacity(1 << 20, f),
        path: tmp.to_path_buf(),
    };
    let mut v = Vec::with_capacity(n);
    while let Some(x) = rr.next()? {
        v.push(x);
    }
    drop(rr);
    std::fs::remove_file(tmp).map_err(err)?;
    if v.len() != n {
        return Err(format!(
            "{}: {} entries read back, {n} written",
            tmp.display(),
            v.len()
        ));
    }
    v.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    Ok(v)
}

fn spill(g: &mut Accs, cfg: &StreamCfg) -> Result<(), String> {
    std::fs::create_dir_all(&cfg.spill_dir)
        .map_err(|e| format!("{}: {e}", cfg.spill_dir.display()))?;
    let seq = g.spills;
    for e in 0..g.maps.len() {
        if g.maps[e].is_empty() {
            continue;
        }
        let m = std::mem::take(&mut g.maps[e]);
        let v = sorted_entries(m, &cfg.spill_dir.join(format!("sort.e{e}.s{seq}.tmp")))?;
        let p = cfg.spill_dir.join(format!("run.e{e}.s{seq}.bin"));
        let f = std::fs::File::create(&p).map_err(|x| format!("{}: {x}", p.display()))?;
        let mut w = BufWriter::with_capacity(1 << 20, f);
        for (k, a) in &v {
            put_run(&mut w, k, a).map_err(|x| format!("{}: {x}", p.display()))?;
        }
        w.flush().map_err(|x| format!("{}: {x}", p.display()))?;
        g.runs[e].push(p);
    }
    g.entries = 0;
    g.spills += 1;
    eprintln!(
        "[arrivals] spill {} written ({}){}",
        g.spills,
        cfg.spill_dir.display(),
        rss()
    );
    Ok(())
}

/// The k-way merge of one edge's sorted in-memory part and its spill runs (equal keys merged).
struct Merger {
    mem: std::vec::IntoIter<(CKey, SAcc)>,
    runs: Vec<RunReader>,
    heap: BinaryHeap<Reverse<(CKey, usize)>>,
    cur: Vec<Option<SAcc>>,
}

impl Merger {
    fn new(mem: Vec<(CKey, SAcc)>, runs: &[PathBuf]) -> Result<Merger, String> {
        let mut m = Merger {
            mem: mem.into_iter(),
            runs: vec![],
            heap: BinaryHeap::new(),
            cur: vec![None; runs.len() + 1],
        };
        for p in runs {
            let f = std::fs::File::open(p).map_err(|e| format!("{}: {e}", p.display()))?;
            m.runs.push(RunReader {
                r: BufReader::with_capacity(1 << 20, f),
                path: p.clone(),
            });
        }
        for s in 0..=m.runs.len() {
            m.advance(s)?;
        }
        Ok(m)
    }

    fn advance(&mut self, s: usize) -> Result<(), String> {
        let nx = if s == 0 {
            self.mem.next()
        } else {
            self.runs[s - 1].next()?
        };
        if let Some((k, a)) = nx {
            self.cur[s] = Some(a);
            self.heap.push(Reverse((k, s)));
        }
        Ok(())
    }

    fn next(&mut self) -> Result<Option<(CKey, SAcc)>, String> {
        let Some(Reverse((k, s))) = self.heap.pop() else {
            return Ok(None);
        };
        let mut acc = self.cur[s].take().expect("current");
        self.advance(s)?;
        while self.heap.peek().is_some_and(|Reverse((k2, _))| *k2 == k) {
            let Reverse((_, s2)) = self.heap.pop().expect("peeked");
            smerge(&mut acc, self.cur[s2].take().expect("current"));
            self.advance(s2)?;
        }
        Ok(Some((k, acc)))
    }
}

// ------------------------------------------------------------------------------------------------------------------
// The walk
// ------------------------------------------------------------------------------------------------------------------

/// The resident memory of this process (progress lines).
pub fn rss() -> String {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let g = |k: &str| {
        s.lines()
            .find(|l| l.starts_with(k))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
            .map_or(0.0, |kb| kb as f64 / 1048576.0)
    };
    format!(" [rss {:.1} GB, peak {:.1} GB]", g("VmRSS:"), g("VmHWM:"))
}

/// What the streamed walk found (complete before the first file is emitted).
pub struct StreamResult {
    pub stats: WalkStats,
    pub closure_digest: String,
    /// Hand-down terms on masks no given edge routes (undecided when `routed_masks` is set).
    pub unrouted: BTreeMap<u64, u64>,
    /// Decided terms whose mask no edge owns (empty when `routed_masks` is the edges' masks).
    pub other: BTreeMap<u64, u64>,
    /// The second-route sample: (served row, decided hand-down terms).
    pub sample: Vec<(usize, Vec<HandTerm>)>,
    pub term_keys: BTreeSet<Vec<i32>>,
    pub spills: u64,
    /// `StreamCfg::trace`: per traced key its closure path back to a start key.
    pub trace: Vec<Value>,
    /// `StreamCfg::closure_out`: what was written (paths, sha256, counts per kind and depth).
    pub closure_out: Option<Value>,
}

/// The closure path of key `k` (`StreamCfg::trace`): steps from `k` back to a start key, each with the key's depth and the
/// closure parent that reached it (parent key, applied key, record sha256, top term); the last step is the start key.
fn closure_path(
    set: &ImportedSet,
    rows: &CompactRows,
    dep: &[u8],
    via: &[(u32, u32)],
    k: &[i32],
) -> Value {
    let Some(mut ki) = rows.find(k) else {
        return json!({"key": k, "in_rows": false});
    };
    if dep[ki] == UNREACHED {
        return json!({"key": k, "in_rows": true, "reached": false});
    }
    let mut steps = vec![];
    while steps.len() <= rows.n_keys() {
        let (si, j) = via[ki];
        let key = rows.key_i32(ki);
        if si == u32::MAX {
            steps.push(json!({"key": key, "depth": dep[ki], "start": true}));
            break;
        }
        let (t, r) = rows.rec(si as usize);
        let pk = rows.served_key(si as usize);
        steps.push(json!({"key": key, "depth": dep[ki], "via": {"parent_key": rows.key_i32(pk),
            "applied": rows.applied(si as usize), "record_sha256": set.rules[t][r].record_sha256, "top_term": j}}));
        ki = pk;
    }
    json!({"key": k, "in_rows": true, "reached": true, "depth": steps.first().map(|s| s["depth"].clone()), "path": steps})
}

/// The exact closure as a key file: magic `RRKEYS01`, u32 LE header length, header JSON, then per reached key in key order:
/// arity x i8 (the node's table frame), u8 chain depth (0 when depths are not tracked), u8 row kind (0 covered by a rule record,
/// 1 terminal, 2 UNCOVERED). The open (uncovered) reached keys also go to `<path>.uncovered.jsonl` ({"key", "depth"} lines).
fn write_closure(
    set: &ImportedSet,
    rows: &CompactRows,
    dep: &[u8],
    path: &Path,
) -> Result<Value, String> {
    use std::io::Write;
    let io = |e: std::io::Error| format!("{}: {e}", path.display());
    let n = set.arity();
    let frame: String = set
        .frame
        .mask()
        .iter()
        .map(|&l| if l { '1' } else { '0' })
        .collect();
    let hdr = json!({"schema": "rustred.campaign.closure-keys.v1", "frame": frame, "arity": n, "rows": rows.path,
        "rows_sha256": rows.sha256,
        "record": format!("{n} x i8 key (table frame), u8 chain depth, u8 kind (0 covered by a rule record, 1 terminal, 2 UNCOVERED)")});
    let hb = hdr.to_string().into_bytes();
    let tmp = path.with_extension("partial");
    let mut w =
        std::io::BufWriter::with_capacity(1 << 22, std::fs::File::create(&tmp).map_err(io)?);
    w.write_all(b"RRKEYS01").map_err(io)?;
    w.write_all(&(hb.len() as u32).to_le_bytes()).map_err(io)?;
    w.write_all(&hb).map_err(io)?;
    let up = PathBuf::from(format!("{}.uncovered.jsonl", path.display()));
    let mut wu = std::io::BufWriter::new(std::fs::File::create(&up).map_err(io)?);
    let mut by_kind = [0u64; 3];
    let mut by_kind_depth: BTreeMap<String, u64> = BTreeMap::new();
    let mut hs = rustred_campaign_core::closure::sha256::Sha256::new();
    for (ki, &d) in dep.iter().enumerate() {
        if d == UNREACHED {
            continue;
        }
        let kd = rows.kind(ki).min(2);
        let mut rec: Vec<u8> = rows.key(ki).iter().map(|&x| x as u8).collect();
        rec.push(d);
        rec.push(kd);
        w.write_all(&rec).map_err(io)?;
        hs.update(&rec);
        by_kind[kd as usize] += 1;
        *by_kind_depth.entry(format!("{kd}/{d}")).or_default() += 1;
        if kd == 2 {
            writeln!(wu, "{}", json!({"key": rows.key_i32(ki), "depth": d})).map_err(io)?;
        }
    }
    w.flush().map_err(io)?;
    drop(w);
    wu.flush().map_err(io)?;
    std::fs::rename(&tmp, path).map_err(io)?;
    Ok(
        json!({"path": path, "uncovered": up, "records_sha256": hs.finish_hex(), "covered": by_kind[0], "terminal": by_kind[1],
        "uncovered_keys": by_kind[2], "by_kind_depth": by_kind_depth}),
    )
}

/// Depth of a key the closure did not reach.
pub const UNREACHED: u8 = u8::MAX;

/// The exact closure CL(S) on key indices (phase A of the module docs of `arrivals`, same rounds and decisions), with the
/// CHAIN DEPTH of every reached key (UNREACHED = not reached): the closure is run in LAYERS of start depth (`start_depth`
/// per key index; None = every start key depth 0): layer d starts from the unreached start keys of depth d and every key it
/// adds gets depth d, so a key's depth is the minimum start depth over the start keys whose closure contains it (top
/// terms add no hand-down step). The reached set is that of a single closure from all start keys.
fn closure(
    set: &ImportedSet,
    rows: &CompactRows,
    cache: &RecCache<'_>,
    cfg: &WalkCfg,
    start_depth: Option<&[u8]>,
    mut via: Option<&mut Vec<(u32, u32)>>,
    st: &mut WalkStats,
) -> Result<Vec<u8>, String> {
    let t0 = std::time::Instant::now();
    let lines = set.frame.mask().to_vec();
    let n = set.arity();
    let nk = rows.n_keys();
    if let Some(sd) = start_depth
        && sd.len() != nk
    {
        return Err(format!(
            "start depths for {} keys, rows have {nk}",
            sd.len()
        ));
    }
    let sdep = |ki: usize| start_depth.map_or(0, |sd| sd[ki]);
    let mut layers: BTreeMap<u8, Vec<u32>> = BTreeMap::new();
    for (ki, s) in rows.in_start.iter().enumerate() {
        if *s {
            let d = sdep(ki);
            if d == UNREACHED {
                return Err(format!(
                    "start key {:?} without a start depth",
                    rows.key_i32(ki)
                ));
            }
            layers.entry(d).or_default().push(ki as u32);
        }
    }
    st.start_keys = layers.values().map(Vec::len).sum::<usize>() as u64;
    let mut dep = vec![UNREACHED; nk];
    if let Some(v) = via.as_deref_mut() {
        *v = vec![(u32::MAX, 0); nk];
    }
    let tops = |si: usize| -> Vec<(u32, Vec<i32>)> {
        let (t, r) = rows.rec(si);
        let (mut buf, mut cm) = (vec![], vec![]);
        let Ok(Some(nt)) = set.bodies.instantiate(
            &set.disp,
            set.disp.rule_ref(t, r),
            &rows.applied(si),
            &mut buf,
            &mut cm,
        ) else {
            return vec![];
        };
        (0..nt)
            .filter_map(|j| {
                let tk = &buf[j * n..(j + 1) * n];
                let (pos, mask) = split_lines(&lines, tk);
                (!pos && mask == 0).then(|| (j as u32, tk.to_vec()))
            })
            .collect()
    };
    let mut cand: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    let mut n_reached = 0usize;
    for (layer, starts) in layers {
        let mut frontier: Vec<u32> = starts
            .into_iter()
            .filter(|&ki| dep[ki as usize] == UNREACHED)
            .collect();
        for &ki in &frontier {
            dep[ki as usize] = layer;
        }
        n_reached += frontier.len();
        while !frontier.is_empty() {
            st.rounds += 1;
            for &ki in &frontier {
                if let Some(si) = rows.served_of(ki as usize) {
                    for (j, tk) in tops(si) {
                        let Some(tki) = rows.find(&tk) else {
                            return Err(format!(
                                "the rows file is not closed: {tk:?} (top term {j} of {:?}) is not a row",
                                rows.key_i32(ki as usize)
                            ));
                        };
                        if dep[tki] == UNREACHED {
                            cand.entry(tki as u32).or_default().push((si as u32, j));
                        }
                    }
                }
            }
            let mut new: Vec<u32> = vec![];
            loop {
                let pend: Vec<(u32, (u32, u32))> = cand
                    .iter()
                    .filter_map(|(k, v)| v.first().map(|c| (*k, *c)))
                    .collect();
                if pend.is_empty() {
                    break;
                }
                let mut by_rec: BTreeMap<RecKey, Vec<PendItem>> = BTreeMap::new();
                for (pi, (_, (si, j))) in pend.iter().enumerate() {
                    by_rec
                        .entry(rows.rec(*si as usize))
                        .or_default()
                        .push((*si as usize, *j, pi));
                }
                let groups: Vec<(RecKey, Vec<PendItem>)> = by_rec.into_iter().collect();
                let res = par_map(
                    &groups,
                    cfg.threads,
                    |((t, r), items)| -> Result<Vec<(usize, CoefFact, bool)>, String> {
                        cache.with(*t, *r, |e| {
                            let mut out = vec![];
                            for (si, j, pi) in items {
                                let applied = rows.applied(*si);
                                let pt =
                                    e.p.point(&applied)
                                        .ok_or_else(|| format!("{applied:?}: no rule point"))?;
                                let (w, by) = exact_word(&mut e.p, &mut e.me, *j as usize, &pt)?;
                                out.push((*pi, w, by == "key:mod"));
                            }
                            Ok(out)
                        })
                    },
                );
                let mut decided = vec![CoefFact::Unknown; pend.len()];
                for r in res {
                    for (pi, w, m) in r? {
                        decided[pi] = w;
                        st.top_decisions += 1;
                        st.top_by_mod += m as u64;
                    }
                }
                for ((k, c), w) in pend.iter().zip(decided) {
                    if w == CoefFact::Zero {
                        st.top_zero += 1;
                        cand.get_mut(k).expect("pending").remove(0);
                    } else {
                        st.top_unknown += (w == CoefFact::Unknown) as u64;
                        cand.remove(k);
                        dep[*k as usize] = layer;
                        if let Some(v) = via.as_deref_mut() {
                            v[*k as usize] = *c;
                        }
                        n_reached += 1;
                        new.push(*k);
                    }
                }
                cand.retain(|_, v| !v.is_empty());
            }
            cand.clear();
            new.sort_unstable();
            eprintln!(
                "[arrivals] closure round {} (depth layer {layer}): +{} keys, reached {n_reached}, top decisions {} ({:.0} s){}",
                st.rounds,
                new.len(),
                st.top_decisions,
                t0.elapsed().as_secs_f64(),
                rss()
            );
            frontier = new;
        }
    }
    cache.release_all();
    st.seconds_closure = t0.elapsed().as_secs_f64();
    Ok(dep)
}

/// Exact-level results of the fallback per child key: (senders proved non-zero, their first).
type Deltas = HashMap<CKey, (u64, Option<First>)>;

/// A fallback item of the exact level: (closure index, edge, child key, first sender, the key's hand-down terms).
type SFallback = (usize, usize, Vec<i32>, First, Arc<Vec<HandTerm>>);

/// What one record group of the fused pass returns.
type GroupOut = (
    WalkStats,
    BTreeMap<u64, u64>,
    Vec<EdgeStats>,
    BTreeMap<u64, u64>,
    Vec<SFallback>,
    Vec<(usize, Vec<HandTerm>)>,
    BTreeSet<Vec<i32>>,
);

/// One edge's lines from the merged accumulators: in blocks, the senders formed in parallel.
struct LineIter<'a> {
    merger: Merger,
    deltas: Deltas,
    set: &'a ImportedSet,
    rows: &'a CompactRows,
    threads: usize,
    buf: VecDeque<Result<ArrivalLine, String>>,
    done: bool,
    /// Write the chain depth of each line (start depths were given).
    depth: bool,
}

const LINE_BLOCK: usize = 1 << 16;

fn make_sender(
    set: &ImportedSet,
    rows: &CompactRows,
    f: First,
    buf: &mut Vec<i32>,
    cm: &mut Vec<u64>,
) -> Result<Sender, String> {
    let n = set.arity();
    let si = f.row as usize;
    let (t, r) = rows.rec(si);
    let applied = rows.applied(si);
    buf.clear();
    cm.clear();
    let nt = match set
        .bodies
        .instantiate(&set.disp, set.disp.rule_ref(t, r), &applied, buf, cm)
    {
        Ok(Some(nt)) => nt,
        _ => {
            return Err(format!(
                "{applied:?}: the first sender's record does not instantiate"
            ));
        }
    };
    let j = f.j as usize;
    if j >= nt {
        return Err(format!("{applied:?}: term {j} of {nt}"));
    }
    Ok(Sender {
        parent_key: rows.key_i32(rows.served_key(si)),
        applied,
        record_sha256: set.rules[t][r].record_sha256.clone(),
        term: f.j,
        term_key: buf[j * n..(j + 1) * n].to_vec(),
        by: by_text(f.by)?,
    })
}

impl LineIter<'_> {
    fn fill(&mut self) -> Result<(), String> {
        let mut block: Vec<(CKey, SAcc)> = Vec::with_capacity(LINE_BLOCK);
        while block.len() < LINE_BLOCK {
            let Some((k, mut a)) = self.merger.next()? else {
                self.done = true;
                break;
            };
            if let Some((add, fe)) = self.deltas.remove(&k) {
                a.exact_senders += add;
                a.first_exact = match (a.first_exact, fe) {
                    (Some(x), Some(y)) => Some(fmin(x, y)),
                    (x, y) => x.or(y),
                };
            }
            block.push((k, a));
        }
        let chunk = block.len().div_ceil(self.threads.max(1)).max(1);
        let parts: Vec<&[(CKey, SAcc)]> = block.chunks(chunk).collect();
        let (set, rows, track) = (self.set, self.rows, self.depth);
        let lines = par_map(
            &parts,
            self.threads,
            |part| -> Vec<Result<ArrivalLine, String>> {
                let (mut buf, mut cm) = (vec![], vec![]);
                part.iter()
                    .map(|(k, a)| {
                        Ok(ArrivalLine {
                            key: k.iter().map(|&x| i32::from(x)).collect(),
                            senders: a.senders,
                            exact_senders: a.exact_level.then_some(a.exact_senders),
                            first: make_sender(set, rows, a.first, &mut buf, &mut cm)?,
                            first_exact: match a.first_exact {
                                Some(f) => Some(make_sender(set, rows, f, &mut buf, &mut cm)?),
                                None => None,
                            },
                            depth: track.then_some(u32::from(a.depth)),
                        })
                    })
                    .collect()
            },
        );
        self.buf.extend(lines.into_iter().flatten());
        Ok(())
    }
}

impl Iterator for LineIter<'_> {
    type Item = Result<ArrivalLine, String>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.buf.is_empty()
            && !self.done
            && let Err(e) = self.fill()
        {
            self.done = true;
            return Some(Err(e));
        }
        self.buf.pop_front()
    }
}

/// The emitter of one edge's file: (the walk's result, edge index, the edge's counters, its lines in key order).
pub type Emit<'e> = dyn FnMut(
        &StreamResult,
        usize,
        EdgeStats,
        &mut dyn Iterator<Item = Result<ArrivalLine, String>>,
    ) -> Result<(), String>
    + 'e;

/// The streamed walk of one parent with all its edges (module docs); `emit` is called once per edge in edge order.
pub fn walk_stream(
    set: &ImportedSet,
    rows: &CompactRows,
    cache: &RecCache<'_>,
    cfg: &WalkCfg,
    scfg: &StreamCfg,
    edges: &[EdgeFile],
    emit: &mut Emit<'_>,
) -> Result<StreamResult, String> {
    let t0 = std::time::Instant::now();
    let n = set.arity();
    let lines = set.frame.mask().to_vec();
    if lines.len() != n {
        return Err(format!("frame of {} slots for arity {n}", lines.len()));
    }
    eprintln!(
        "[arrivals] streamed walk: {} rows ({} served){}",
        rows.n_keys(),
        rows.n_served(),
        rss()
    );
    let mut st = WalkStats {
        rows_keys: rows.n_keys() as u64,
        rows_served: rows.n_served() as u64,
        ..Default::default()
    };
    // --- A ---
    let mut via: Vec<(u32, u32)> = vec![];
    let dep = closure(
        set,
        rows,
        cache,
        cfg,
        scfg.start_depth.as_deref(),
        (!scfg.trace.is_empty()).then_some(&mut via),
        &mut st,
    )?;
    let trace: Vec<Value> = scfg
        .trace
        .iter()
        .map(|k| closure_path(set, rows, &dep, &via, k))
        .collect();
    drop(via);
    let closure_out = match &scfg.closure_out {
        Some(p) => Some(write_closure(set, rows, &dep, p)?),
        None => None,
    };
    st.closure_keys = dep.iter().filter(|x| **x != UNREACHED).count() as u64;
    st.keys_only_not_reached = rows.n_keys() as u64 - st.closure_keys;
    let mut h = rustred_campaign_core::closure::sha256::Sha256::new();
    for (ki, d) in dep.iter().enumerate() {
        if *d != UNREACHED {
            h.update(format!("{:?}\n", rows.key_i32(ki)).as_bytes());
        }
    }
    let closure_digest = format!("sha256:{}", h.finish_hex());
    let cl_served: Vec<u32> = (0..rows.n_served())
        .filter(|&si| dep[rows.served_key(si)] != UNREACHED)
        .map(|si| si as u32)
        .collect();
    // the depth an arrival from served closure row ci gets: its key's depth + 1 (one hand-down step)
    let cl_depth: Vec<u8> = cl_served
        .iter()
        .map(|&si| {
            dep[rows.served_key(si as usize)]
                .saturating_add(1)
                .min(UNREACHED - 1)
        })
        .collect();
    if scfg.start_depth.is_some() {
        for d in dep.iter().filter(|d| **d != UNREACHED) {
            *st.depth_hist.entry(*d as u32).or_default() += 1;
        }
    }
    drop(dep);
    st.closure_served = cl_served.len() as u64;
    if scfg.closure_only {
        return Ok(StreamResult {
            stats: st,
            closure_digest,
            unrouted: BTreeMap::new(),
            other: BTreeMap::new(),
            sample: vec![],
            term_keys: BTreeSet::new(),
            spills: 0,
            trace,
            closure_out,
        });
    }
    // --- B + C fused, per record group ---
    let t1 = std::time::Instant::now();
    let exp = ExpShards::new(cfg.cache_entries);
    let owners = mask_owners(edges)?;
    let alg = cache.alg;
    let child_lines: Vec<Vec<usize>> = edges.iter().map(|e| e.child.table_lines.clone()).collect();
    let fields: Vec<Zp64> = PRIMES.iter().map(|(p, _)| Zp64::new(*p)).collect();
    let cx = TransportCtx {
        owners: &owners,
        edges,
        child_lines: &child_lines,
        fields: &fields,
        exp: &exp,
        cfg,
    };
    let sample_step = cl_served
        .len()
        .checked_div(scfg.decisions_sample)
        .unwrap_or(1)
        .max(1);
    let sampled = |ci: usize| {
        scfg.decisions_sample > 0
            && ci.is_multiple_of(sample_step)
            && ci / sample_step < scfg.decisions_sample
    };
    let mut by_rec: BTreeMap<RecKey, Vec<usize>> = BTreeMap::new();
    for (ci, &si) in cl_served.iter().enumerate() {
        by_rec.entry(rows.rec(si as usize)).or_default().push(ci);
    }
    let mut groups: Vec<(RecKey, Vec<usize>)> = by_rec.into_iter().collect();
    // most expensive groups first (record bytes x (keys + a first-use allowance)): a heavy record with few keys must not
    // start last and run alone (the work order does not change any output: accumulators merge order-free)
    groups.sort_by_key(|((t, r), ks)| {
        Reverse((
            u128::from(set.rules[*t][*r].bytes) * (ks.len() as u128 + 64),
            ks.len(),
        ))
    });
    st.records_walked = groups.len() as u64;
    let accs = Mutex::new(Accs {
        maps: edges.iter().map(|_| HashMap::new()).collect(),
        entries: 0,
        runs: edges.iter().map(|_| vec![]).collect(),
        spills: 0,
    });
    // work items = chunks of CHUNK keys of a record group (most expensive groups first); a worker decides a chunk holding
    // the record (one at a time per record), releases it, then transports the chunk; the record's parsed form is freed
    // when its last chunk is decided. Long-lived workers (one per thread) keep their accumulators across items and
    // merge them into the shared ones past LOCAL_FLUSH entries and at their end.
    // interleaved: every group's first chunk (most expensive groups first), then every second chunk, ...: concurrent workers
    // mostly hold different records (chunks of one record are decided one at a time)
    let max_chunks = groups
        .iter()
        .map(|(_, ks)| ks.len().div_ceil(CHUNK))
        .max()
        .unwrap_or(0);
    let mut items: Vec<(usize, usize, usize)> = vec![];
    for c in 0..max_chunks {
        for (g, (_, ks)) in groups.iter().enumerate() {
            let a = c * CHUNK;
            if a < ks.len() {
                items.push((g, a, (a + CHUNK).min(ks.len())));
            }
        }
    }
    let left: Vec<AtomicUsize> = groups
        .iter()
        .map(|(_, ks)| AtomicUsize::new(ks.len().div_ceil(CHUNK)))
        .collect();
    let next_item = AtomicUsize::new(0);
    // progress: a line each time another 5 % of the work items are decided
    let items_done = AtomicUsize::new(0);
    let step = items.len().div_ceil(20).max(1);
    let worker = || -> Result<GroupOut, String> {
        let tr = Transporter::new(alg, n)?;
        let mut s = WalkStats::default();
        let mut unr: BTreeMap<u64, u64> = BTreeMap::new();
        let mut est: Vec<EdgeStats> = edges.iter().map(|_| EdgeStats::default()).collect();
        let mut other: BTreeMap<u64, u64> = BTreeMap::new();
        let mut fallback: Vec<SFallback> = vec![];
        let mut sample = vec![];
        let mut tks: BTreeSet<Vec<i32>> = BTreeSet::new();
        let mut local: Vec<HashMap<CKey, SAcc>> = edges.iter().map(|_| HashMap::new()).collect();
        let mut local_n = 0usize;
        while let Some(&(g, a, b)) = items.get(next_item.fetch_add(1, Ordering::Relaxed)) {
            let ((t, r), ks) = &groups[g];
            let use_structural = cfg.structural && ks.len() >= cfg.structural_min_keys;
            let decided: Vec<(usize, u32, Vec<HandTerm>)> = cache.with(*t, *r, |ent| {
                let (p, me) = (&mut ent.p, &mut ent.me);
                let mut out = Vec::with_capacity(b - a);
                for &ci in &ks[a..b] {
                    let si = cl_served[ci];
                    let applied = rows.applied(si as usize);
                    let pt = p
                        .point(&applied)
                        .ok_or_else(|| format!("{applied:?}: no rule point"))?;
                    let hand = decide_hand(
                        p,
                        me,
                        cfg,
                        use_structural,
                        &lines,
                        &pt,
                        &applied,
                        &mut s,
                        &mut unr,
                    )?;
                    out.push((ci, si, hand));
                }
                let _ = p.take_new();
                Ok(out)
            })?;
            let k = items_done.fetch_add(1, Ordering::Relaxed) + 1;
            if k.is_multiple_of(step) || k == items.len() {
                eprintln!(
                    "[arrivals] hand-downs: {k}/{} work items decided ({:.0} s){}",
                    items.len(),
                    t1.elapsed().as_secs_f64(),
                    rss()
                );
            }
            if left[g].fetch_sub(1, Ordering::AcqRel) == 1 {
                cache.release(*t, *r);
            }
            for (ci, si, hand) in decided {
                if sampled(ci) {
                    sample.push((si as usize, hand.clone()));
                }
                if scfg.term_keys {
                    for h in &hand {
                        if owners.contains_key(&h.mask) {
                            tks.insert(h.term_key());
                        }
                    }
                }
                let ids: Vec<u32> = hand
                    .iter()
                    .map(|h| by_id(&h.by))
                    .collect::<Result<_, _>>()?;
                let mut sent = vec![false; edges.len()];
                let mut per =
                    transport_hand(&cx, &tr, &hand, &mut est, &mut other, &mut sent, |hi, h| {
                        First {
                            row: si,
                            j: h.j,
                            by: ids[hi],
                        }
                    })?;
                let mut shared: Option<Arc<Vec<HandTerm>>> = None;
                for e in 0..edges.len() {
                    est[e].sending_parent_keys += sent[e] as u64;
                    for (k, pk) in per[e].drain() {
                        let wants = cfg.exact.wants(&k);
                        let m = local[e].entry(ck(&k)?).or_insert_with(|| {
                            local_n += 1;
                            SAcc {
                                senders: 0,
                                exact_senders: 0,
                                first: pk.first,
                                first_exact: None,
                                exact_level: false,
                                depth: cl_depth[ci],
                            }
                        });
                        m.depth = m.depth.min(cl_depth[ci]);
                        m.senders += 1;
                        m.first = fmin(m.first, pk.first);
                        if !wants {
                            continue;
                        }
                        est[e].exact_pairs += 1;
                        m.exact_level = true;
                        let witness = pk
                            .acc
                            .iter()
                            .enumerate()
                            .any(|(i, v)| v.as_ref().is_some_and(|v| !fields[i].is_zero(v)));
                        if witness {
                            est[e].exact_by_witness += 1;
                            m.exact_senders += 1;
                            m.first_exact =
                                Some(m.first_exact.map_or(pk.first, |f| fmin(f, pk.first)));
                        } else {
                            let hs = shared.get_or_insert_with(|| Arc::new(hand.clone())).clone();
                            fallback.push((ci, e, k, pk.first, hs));
                        }
                    }
                }
                if local_n > LOCAL_FLUSH {
                    merge_into(&accs, &mut local, scfg)?;
                    local_n = 0;
                }
            }
        }
        merge_into(&accs, &mut local, scfg)?;
        Ok((s, unr, est, other, fallback, sample, tks))
    };
    let res: Vec<Result<GroupOut, String>> = std::thread::scope(|sc| {
        let hs: Vec<_> = (0..cfg.threads.max(1)).map(|_| sc.spawn(worker)).collect();
        hs.into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err("a worker panicked".into())))
            .collect()
    });
    let mut est: Vec<EdgeStats> = edges.iter().map(|_| EdgeStats::default()).collect();
    let mut unrouted: BTreeMap<u64, u64> = BTreeMap::new();
    let mut other: BTreeMap<u64, u64> = BTreeMap::new();
    let mut fallback: Vec<SFallback> = vec![];
    let mut sample: Vec<(usize, Vec<HandTerm>)> = vec![];
    let mut term_keys: BTreeSet<Vec<i32>> = BTreeSet::new();
    for r in res {
        let (s, u, es, ot, fb, sm, tk) = r?;
        for (m, c) in u {
            *unrouted.entry(m).or_default() += c;
        }
        st.handdown_unrouted += s.handdown_unrouted;
        st.handdown_terms += s.handdown_terms;
        st.handdown_zero += s.handdown_zero;
        st.handdown_nonzero += s.handdown_nonzero;
        st.handdown_unknown += s.handdown_unknown;
        st.by_structural_zero += s.by_structural_zero;
        st.by_structural_nonzero += s.by_structural_nonzero;
        st.by_key_mod += s.by_key_mod;
        st.by_key_exact += s.by_key_exact;
        st.positive_numerator_terms += s.positive_numerator_terms;
        for (e, s) in es.into_iter().enumerate() {
            let t = &mut est[e];
            t.sending_parent_keys += s.sending_parent_keys;
            t.handdown_terms_routed += s.handdown_terms_routed;
            t.image_terms += s.image_terms;
            t.lower_child_sector_terms += s.lower_child_sector_terms;
            t.exact_pairs += s.exact_pairs;
            t.exact_by_witness += s.exact_by_witness;
            t.unknown_terms_sending += s.unknown_terms_sending;
        }
        for (m, c) in ot {
            *other.entry(m).or_default() += c;
        }
        fallback.extend(fb);
        sample.extend(sm);
        term_keys.extend(tk);
    }
    sample.sort_by_key(|x| x.0);
    st.seconds_handdowns = t1.elapsed().as_secs_f64();
    eprintln!(
        "[arrivals] hand-down decisions: {} terms ({} zero, {} non-zero) at {} keys ({:.0} s){}",
        st.handdown_terms,
        st.handdown_zero,
        st.handdown_nonzero,
        cl_served.len(),
        st.seconds_handdowns,
        rss()
    );
    // --- the exact symbolic fallback, grouped by record; its results merged as deltas at the end ---
    let mut deltas: Vec<Deltas> = edges.iter().map(|_| HashMap::new()).collect();
    if !fallback.is_empty() {
        let mut by_rec: BTreeMap<RecKey, Vec<SFallback>> = BTreeMap::new();
        for x in fallback {
            by_rec
                .entry(rows.rec(cl_served[x.0] as usize))
                .or_default()
                .push(x);
        }
        let groups: Vec<_> = by_rec.into_iter().collect();
        type FbOut = (usize, Vec<i32>, First, bool);
        let res = par_map(
            &groups,
            cfg.threads,
            |((t, r), items)| -> Result<Vec<FbOut>, String> {
                let tr = Transporter::new(alg, n)?;
                cache
                    .with(*t, *r, |ent| {
                        let p = &mut ent.p;
                        let mut out = vec![];
                        for (ci, e, k, first, hand) in items {
                            let applied = rows.applied(cl_served[*ci] as usize);
                            let pt = p.point(&applied).ok_or("no rule point")?;
                            let nz = fallback_nonzero(p, &tr, edges, &owners, *e, k, &pt, hand)?;
                            out.push((*e, k.clone(), *first, nz));
                        }
                        Ok(out)
                    })
                    .inspect(|_| cache.release(*t, *r))
            },
        );
        for r in res {
            for (e, k, first, nz) in r? {
                if nz {
                    est[e].exact_by_symbolic += 1;
                    let d = deltas[e].entry(ck(&k)?).or_insert((0, None));
                    d.0 += 1;
                    d.1 = Some(d.1.map_or(first, |f| fmin(f, first)));
                } else {
                    est[e].exact_cancelled += 1;
                }
            }
        }
    }
    let mut accs = accs
        .into_inner()
        .map_err(|_| "accumulator lock".to_string())?;
    let result = StreamResult {
        stats: st,
        closure_digest,
        unrouted,
        other,
        sample,
        term_keys,
        spills: accs.spills,
        trace,
        closure_out,
    };
    // --- one edge at a time: merge (memory part + runs), stream its file ---
    for (e, est_e) in est.into_iter().enumerate() {
        let m = std::mem::take(&mut accs.maps[e]);
        if !m.is_empty() {
            std::fs::create_dir_all(&scfg.spill_dir)
                .map_err(|x| format!("{}: {x}", scfg.spill_dir.display()))?;
        }
        let mem = sorted_entries(m, &scfg.spill_dir.join(format!("sort.e{e}.final.tmp")))?;
        let runs = std::mem::take(&mut accs.runs[e]);
        let mut it = LineIter {
            merger: Merger::new(mem, &runs)?,
            deltas: std::mem::take(&mut deltas[e]),
            set,
            rows,
            threads: cfg.threads,
            buf: VecDeque::new(),
            done: false,
            depth: scfg.start_depth.is_some(),
        };
        let mut s = est_e;
        s.seconds = t0.elapsed().as_secs_f64();
        emit(&result, e, s, &mut it)?;
        if !it.deltas.is_empty() {
            return Err(format!(
                "{} exact-level results without their key",
                it.deltas.len()
            ));
        }
        drop(it);
        for p in &runs {
            std::fs::remove_file(p).map_err(|x| format!("{}: {x}", p.display()))?;
        }
    }
    eprintln!(
        "[arrivals] files written ({:.0} s){}",
        t0.elapsed().as_secs_f64(),
        rss()
    );
    Ok(result)
}
