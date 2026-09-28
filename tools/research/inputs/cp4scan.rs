// Read-only streaming scan of a CP3/CP4 walk checkpoint state file (framed JSON).
//
// The CP4 image (binary 32fdec, commit 01f8db5d) is one serde_json document
// `{"descendant_closure":{"nodes":[..],"edges":[{"source","target","next"}..],..},
//   "queue":[metadata, domains, buckets, ledger], "records":[..], ...}`
// written through length-prefixed 64 KiB frames (every frame but the last is
// full; checked on a sample before the parallel phase). This tool parses only
// what it needs and skips everything else byte-wise. The edge array (the bulk
// of the file) is parsed in parallel: its objects contain no nested braces,
// so a thread starting mid-array resynchronises at the next '{'.
//
// Outputs (OUT_DIR):
//   frontiers.tsv    one row per record with a nonempty `frontiers` array
//   transitions.tsv  owner-level dependency-edge counts, census.rs keys:
//                    Apply owner index (OWNERS_TXT line), 100000+mask for a
//                    non-owner Apply mask, 200000+mask for Route
//   ancestors.tsv    per key: nodes reverse-reachable from a node whose record
//                    carries a local_dispatch_frontier (seeds included)
//   seed_ancestors.tsv  per seed owner (key of the frontier node): the Apply
//                    owner keys among its ancestors (one reverse walk per
//                    seed key)
//   apply_census.tsv per key: Apply domains, unbounded-A count, max finite A,
//                    max finite rank, unbounded-rank count, inspected count
//   summary.json     counts and timings
//
// Usage: cp4scan STATE_FILE OWNERS_TXT OUT_DIR [THREADS]
//   OWNERS_TXT: one owner mask per line in selection order.
// Mask strings use character i = coordinate i, as in the JSON documents; the
// internal u16 has bit i = coordinate i.
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom, Write};
use std::time::Instant;

const N: usize = 15;

/// Fast hasher for integer keys (std's SipHash is slow for 4e9 inserts).
#[derive(Default)]
struct Fx(u64);
impl std::hash::Hasher for Fx {
    fn finish(&self) -> u64 {
        // murmur3 fmix64: spreads high key bits into the bucket bits
        let mut h = self.0;
        h ^= h >> 33;
        h = h.wrapping_mul(0xff51afd7ed558ccd);
        h ^= h >> 33;
        h = h.wrapping_mul(0xc4ceb9fe1a85ec53);
        h ^ (h >> 33)
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_u64(&mut self, x: u64) {
        self.0 = self.0.rotate_left(29) ^ x;
    }
    fn write_u32(&mut self, x: u32) {
        self.write_u64(x as u64);
    }
}
type FxMap<K, V> = HashMap<K, V, std::hash::BuildHasherDefault<Fx>>;
const FRAME: usize = 65536;
const MAGIC_LEN: u64 = 17;

struct Src {
    file: BufReader<File>,
    buf: Vec<u8>,
    pos: usize,
    len: usize,
    done: bool,
    /// Index of the frame held in `buf`; `u64::MAX` before the first one.
    frame: u64,
}

impl Src {
    fn check_magic(path: &str) {
        let mut f = File::open(path).expect("open state");
        let mut magic = [0u8; MAGIC_LEN as usize];
        f.read_exact(&mut magic).unwrap();
        assert!(
            &magic == b"RUSTRED-WALK-CP4\n" || &magic == b"RUSTRED-WALK-CP3\n",
            "not a CP3/CP4 state file"
        );
    }
    /// Position at deframed byte offset `d` (requires regular frames).
    fn open_at(path: &str, d: u64, buffer: usize) -> Src {
        let mut f = File::open(path).expect("open state");
        let k = d / FRAME as u64;
        f.seek(SeekFrom::Start(MAGIC_LEN + k * (FRAME as u64 + 4))).unwrap();
        let file = BufReader::with_capacity(buffer, f);
        let mut s = Src { file, buf: vec![0u8; FRAME], pos: 0, len: 0, done: false, frame: k.wrapping_sub(1) };
        assert!(s.refill(), "offset beyond the last frame");
        s.pos = (d % FRAME as u64) as usize;
        s
    }
    fn deframed(&self) -> u64 {
        self.frame * FRAME as u64 + self.pos as u64
    }
    #[inline(never)]
    fn refill(&mut self) -> bool {
        if self.done {
            return false;
        }
        let mut h = [0u8; 4];
        self.file.read_exact(&mut h).unwrap();
        let n = u32::from_le_bytes(h) as usize;
        if n == 0 {
            self.done = true;
            return false;
        }
        assert!(n <= FRAME, "frame bound");
        self.file.read_exact(&mut self.buf[..n]).unwrap();
        self.pos = 0;
        self.len = n;
        self.frame = self.frame.wrapping_add(1);
        true
    }
    #[inline(always)]
    fn peek(&mut self) -> u8 {
        if self.pos == self.len && !self.refill() {
            return 0;
        }
        self.buf[self.pos]
    }
    #[inline(always)]
    fn next(&mut self) -> u8 {
        let c = self.peek();
        self.pos += 1;
        c
    }
    #[inline(always)]
    fn ws(&mut self) -> u8 {
        loop {
            let c = self.peek();
            if c == b' ' || c == b'\n' || c == b'\t' || c == b'\r' {
                self.pos += 1;
            } else {
                return c;
            }
        }
    }
    fn expect(&mut self, want: u8) {
        let c = self.ws();
        assert_eq!(c as char, want as char, "unexpected byte at deframed {}", self.deframed());
        self.pos += 1;
    }
    /// Short string (keys, enum names); escapes are kept verbatim.
    fn string(&mut self) -> Vec<u8> {
        self.expect(b'"');
        let mut out = Vec::with_capacity(32);
        loop {
            let c = self.next();
            match c {
                b'"' => return out,
                b'\\' => {
                    out.push(c);
                    out.push(self.next());
                }
                _ => out.push(c),
            }
        }
    }
    fn skip_string_body(&mut self) {
        loop {
            if self.pos == self.len && !self.refill() {
                panic!("eof in string");
            }
            let slice = &self.buf[self.pos..self.len];
            match slice.iter().position(|&b| b == b'"' || b == b'\\') {
                Some(i) => {
                    self.pos += i;
                    let c = self.next();
                    if c == b'"' {
                        return;
                    }
                    self.next();
                }
                None => self.pos = self.len,
            }
        }
    }
    /// Skip one JSON value of any type.
    fn skip(&mut self) {
        let c = self.ws();
        match c {
            b'"' => {
                self.pos += 1;
                self.skip_string_body();
            }
            b'{' | b'[' => {
                let mut depth = 0usize;
                loop {
                    if self.pos == self.len && !self.refill() {
                        panic!("eof in container");
                    }
                    let c = self.buf[self.pos];
                    self.pos += 1;
                    match c {
                        b'{' | b'[' => depth += 1,
                        b'}' | b']' => {
                            depth -= 1;
                            if depth == 0 {
                                return;
                            }
                        }
                        b'"' => self.skip_string_body(),
                        _ => {}
                    }
                }
            }
            _ => loop {
                let c = self.peek();
                if c == b',' || c == b'}' || c == b']' || c == 0 || c == b' ' || c == b'\n' {
                    return;
                }
                self.pos += 1;
            },
        }
    }
    /// Unsigned integer, or None for `null`.
    fn opt_u64(&mut self) -> Option<u64> {
        let c = self.ws();
        if c == b'n' {
            for _ in 0..4 {
                self.next();
            }
            return None;
        }
        let mut v: u64 = 0;
        loop {
            let c = self.peek();
            if c.is_ascii_digit() {
                v = v * 10 + (c - b'0') as u64;
                self.pos += 1;
            } else {
                break;
            }
        }
        Some(v)
    }
    fn opt_i64(&mut self) -> Option<i64> {
        let c = self.ws();
        if c == b'-' {
            self.pos += 1;
            return self.opt_u64().map(|v| -(v as i64));
        }
        self.opt_u64().map(|v| v as i64)
    }
    fn boolean(&mut self) -> bool {
        let c = self.ws();
        let n = if c == b't' { 4 } else { 5 };
        for _ in 0..n {
            self.next();
        }
        c == b't'
    }
    /// Iterate an array: returns false at the closing bracket.
    fn array_next(&mut self, first: &mut bool) -> bool {
        let c = self.ws();
        if *first {
            *first = false;
            if c == b']' {
                self.pos += 1;
                return false;
            }
            return true;
        }
        self.pos += 1;
        match c {
            b',' => true,
            b']' => false,
            _ => panic!("array separator {} at deframed {}", c as char, self.deframed()),
        }
    }
    /// Iterate an object: returns the next key or None at the closing brace.
    fn object_next(&mut self, first: &mut bool) -> Option<Vec<u8>> {
        let c = self.ws();
        if *first {
            *first = false;
            if c == b'}' {
                self.pos += 1;
                return None;
            }
        } else {
            self.pos += 1;
            match c {
                b',' => {}
                b'}' => return None,
                _ => panic!("object separator {} at deframed {}", c as char, self.deframed()),
            }
        }
        let k = self.string();
        self.expect(b':');
        Some(k)
    }
}

#[derive(Clone, Copy, Default)]
struct Dom {
    phase: u8,  // 0 Apply 1 Route
    mask: u16,  // bit i = coordinate i
    rank: u8,   // 255 = None
    maxpos: u8, // 255 = None (saturating at 254)
    lower_sum_active: u16,
    has_finite_upper: bool,
}

fn mask_from_str(s: &[u8]) -> u16 {
    let mut m = 0u16;
    for (i, &c) in s.iter().enumerate() {
        if c == b'1' {
            m |= 1 << i;
        }
    }
    m
}
fn mask_str(m: u16) -> String {
    (0..N).map(|i| if m & (1 << i) != 0 { '1' } else { '0' }).collect()
}

/// Bytes that occur in the edge array and nowhere else in a full frame.
fn edge_byte(b: u8) -> bool {
    matches!(b, b'{' | b'}' | b'"' | b':' | b',' | b'0'..=b'9' | b's' | b'o' | b'u' | b'r' | b'c' | b'e' | b't' | b'a' | b'g' | b'n' | b'x')
}

fn read_frame(f: &mut File, k: u64, buf: &mut Vec<u8>) -> usize {
    f.seek(SeekFrom::Start(MAGIC_LEN + k * (FRAME as u64 + 4))).unwrap();
    let mut h = [0u8; 4];
    f.read_exact(&mut h).unwrap();
    let n = u32::from_le_bytes(h) as usize;
    buf.resize(n, 0);
    f.read_exact(&mut buf[..n]).unwrap();
    n
}

/// Parse `{"source":a,"target":b,...}` objects whose '{' lies in [start, end).
fn parse_edges(path: &str, start: u64, end: u64) -> (Vec<u32>, Vec<u32>) {
    let mut src = Src::open_at(path, start, 8 << 20);
    let cap = ((end - start) / 48) as usize;
    let (mut s_out, mut t_out) = (Vec::with_capacity(cap), Vec::with_capacity(cap));
    // resynchronise at the first object start
    while src.peek() != b'{' {
        src.pos += 1;
    }
    loop {
        if src.deframed() >= end {
            break;
        }
        src.expect(b'{');
        let mut fo = true;
        let (mut s, mut t) = (u64::MAX, u64::MAX);
        while let Some(k) = src.object_next(&mut fo) {
            match k.as_slice() {
                b"source" => s = src.opt_u64().unwrap(),
                b"target" => t = src.opt_u64().unwrap(),
                _ => src.skip(),
            }
        }
        assert!(s < u32::MAX as u64 && t < u32::MAX as u64, "edge endpoint");
        s_out.push(s as u32);
        t_out.push(t as u32);
        match src.ws() {
            b',' => src.pos += 1,
            b']' => break,
            c => panic!("edge separator {} at {}", c as char, src.deframed()),
        }
    }
    (s_out, t_out)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let state = args[1].clone();
    let owners_txt = std::fs::read_to_string(&args[2]).unwrap();
    let out = &args[3];
    let threads: usize = args.get(4).map(|s| s.parse().unwrap()).unwrap_or(16);
    std::fs::create_dir_all(out).unwrap();
    let owner_masks: Vec<u16> =
        owners_txt.lines().filter(|l| !l.trim().is_empty()).map(|l| mask_from_str(l.trim().as_bytes())).collect();
    let owner_of: HashMap<u16, u32> = owner_masks.iter().enumerate().map(|(i, &m)| (m, i as u32)).collect();
    let t0 = Instant::now();
    Src::check_magic(&state);
    let size = std::fs::metadata(&state).unwrap().len();
    let nframes = (size - MAGIC_LEN).div_ceil(FRAME as u64 + 4);
    // Frame regularity: every frame but the last is full.
    {
        let mut f = File::open(&state).unwrap();
        let mut buf = Vec::new();
        let step = (nframes / 4096).max(1);
        let mut k = 0;
        while k + 1 < nframes {
            assert_eq!(read_frame(&mut f, k, &mut buf), FRAME, "irregular frame {k}");
            k += step;
        }
    }

    // ---- sequential: nodes, then locate the edge array
    let mut src = Src::open_at(&state, 0, 64 << 20);
    let mut flags: Vec<u8> = Vec::new(); // bit0 sealed, bit1 inspected, bit2 closed
    src.expect(b'{');
    let mut first_top = true;
    let key0 = src.object_next(&mut first_top).unwrap();
    assert_eq!(key0, b"descendant_closure");
    src.expect(b'{');
    let mut first_dc = true;
    let edges_start;
    loop {
        let k1 = src.object_next(&mut first_dc).expect("edges key");
        match k1.as_slice() {
            b"nodes" => {
                src.expect(b'[');
                let mut fa = true;
                while src.array_next(&mut fa) {
                    src.expect(b'{');
                    let mut fo = true;
                    let mut fl = 0u8;
                    while let Some(k) = src.object_next(&mut fo) {
                        match k.as_slice() {
                            b"sealed" => fl |= src.boolean() as u8,
                            b"inspected" => fl |= (src.boolean() as u8) << 1,
                            b"closed" => fl |= (src.boolean() as u8) << 2,
                            _ => src.skip(),
                        }
                    }
                    flags.push(fl);
                }
                eprintln!("nodes {} ({:.1}s)", flags.len(), t0.elapsed().as_secs_f64());
            }
            b"edges" => {
                src.expect(b'[');
                edges_start = src.deframed();
                break;
            }
            _ => src.skip(),
        }
    }
    drop(src);
    // ---- binary search for the first frame that is not pure edge text
    let f0 = edges_start / FRAME as u64 + 1;
    let pure = |k: u64| -> bool {
        let mut f = File::open(&state).unwrap();
        let mut buf = Vec::new();
        let n = read_frame(&mut f, k, &mut buf);
        n == FRAME && buf.iter().all(|&b| edge_byte(b))
    };
    let (mut lo, mut hi) = (f0, nframes); // invariant: pure(lo-1) (or lo == f0), !pure(hi) or hi == nframes
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if pure(mid) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    let first_impure = lo;
    assert!(first_impure < nframes, "edge section reaches the file end");
    for k in first_impure.saturating_sub(64).max(f0)..first_impure {
        assert!(pure(k), "edge section not contiguous near frame {k}");
    }
    let edges_end = {
        let mut f = File::open(&state).unwrap();
        let mut buf = Vec::new();
        read_frame(&mut f, first_impure, &mut buf);
        let i = buf.iter().position(|&b| b == b']').expect("closing bracket");
        first_impure * FRAME as u64 + i as u64
    };
    eprintln!("edge array deframed [{edges_start}, {edges_end}) ({:.1}s)", t0.elapsed().as_secs_f64());

    // ---- parallel edges
    let span = edges_end - edges_start;
    let parts: Vec<(u64, u64)> = (0..threads as u64)
        .map(|i| (edges_start + span * i / threads as u64, edges_start + span * (i + 1) / threads as u64))
        .collect();
    let edge_parts: Vec<(Vec<u32>, Vec<u32>)> = std::thread::scope(|sc| {
        let hs: Vec<_> = parts.iter().map(|&(a, b)| { let st = state.clone(); sc.spawn(move || parse_edges(&st, a, b)) }).collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let edge_count: usize = edge_parts.iter().map(|p| p.0.len()).sum();
    eprintln!("edges {} ({:.1}s)", edge_count, t0.elapsed().as_secs_f64());

    // ---- sequential tail: rest of descendant_closure, queue domains, records
    let mut src = Src::open_at(&state, edges_end, 64 << 20);
    src.expect(b']');
    while let Some(_k) = src.object_next(&mut first_dc) {
        src.skip();
    }
    let mut doms: Vec<Dom> = Vec::new();
    let mut frontier_rows: Vec<(u64, Vec<(Vec<u8>, u64)>)> = Vec::new();
    let mut record_count = 0u64;
    let mut kind_totals: HashMap<Vec<u8>, u64> = HashMap::new();
    while let Some(key) = src.object_next(&mut first_top) {
        eprintln!("top key {} at deframed {} ({:.1}s)", String::from_utf8_lossy(&key), src.deframed(), t0.elapsed().as_secs_f64());
        match key.as_slice() {
            b"queue" => {
                src.expect(b'[');
                let mut fa = true;
                let mut idx = 0;
                while src.array_next(&mut fa) {
                    if idx == 1 {
                        src.expect(b'[');
                        let mut fd = true;
                        while src.array_next(&mut fd) {
                            src.expect(b'{');
                            let mut fo = true;
                            let mut d = Dom { rank: 255, maxpos: 255, ..Default::default() };
                            let mut lower = [0u64; N];
                            while let Some(k) = src.object_next(&mut fo) {
                                match k.as_slice() {
                                    b"phase" => {
                                        let s = src.string();
                                        d.phase = if s == b"Apply" { 0 } else { 1 };
                                    }
                                    b"owner" => {
                                        src.expect(b'[');
                                        let mut fb = true;
                                        let mut i = 0;
                                        while src.array_next(&mut fb) {
                                            if src.boolean() {
                                                d.mask |= 1 << i;
                                            }
                                            i += 1;
                                        }
                                    }
                                    b"lower" => {
                                        src.expect(b'[');
                                        let mut fb = true;
                                        let mut i = 0;
                                        while src.array_next(&mut fb) {
                                            lower[i] = src.opt_u64().unwrap();
                                            i += 1;
                                        }
                                    }
                                    b"upper" => {
                                        src.expect(b'[');
                                        let mut fb = true;
                                        while src.array_next(&mut fb) {
                                            if src.opt_u64().is_some() {
                                                d.has_finite_upper = true;
                                            }
                                        }
                                    }
                                    b"rank" => d.rank = src.opt_u64().map(|r| r.min(254) as u8).unwrap_or(255),
                                    b"powers" => {
                                        src.expect(b'[');
                                        let mut fb = true;
                                        let mut i = 0;
                                        while src.array_next(&mut fb) {
                                            let v = src.opt_i64();
                                            if i == 0 {
                                                d.maxpos = v.map(|x| x.clamp(0, 254) as u8).unwrap_or(255);
                                            }
                                            i += 1;
                                        }
                                    }
                                    _ => src.skip(),
                                }
                            }
                            d.lower_sum_active =
                                (0..N).filter(|&i| d.mask & (1 << i) != 0).map(|i| lower[i]).sum::<u64>().min(65535) as u16;
                            doms.push(d);
                        }
                        eprintln!("domains {} ({:.1}s)", doms.len(), t0.elapsed().as_secs_f64());
                    } else {
                        src.skip();
                    }
                    idx += 1;
                }
            }
            b"records" => {
                src.expect(b'[');
                let mut fa = true;
                while src.array_next(&mut fa) {
                    record_count += 1;
                    src.expect(b'{');
                    let mut fo = true;
                    let mut id = u64::MAX;
                    let mut kinds: Vec<(Vec<u8>, u64)> = Vec::new();
                    while let Some(k) = src.object_next(&mut fo) {
                        match k.as_slice() {
                            b"id" => id = src.opt_u64().unwrap(),
                            b"frontiers" => {
                                src.expect(b'[');
                                let mut ff = true;
                                while src.array_next(&mut ff) {
                                    if src.ws() != b'{' {
                                        src.skip();
                                        continue;
                                    }
                                    src.expect(b'{');
                                    let mut fk = true;
                                    let mut kind = b"?".to_vec();
                                    while let Some(k2) = src.object_next(&mut fk) {
                                        if k2 == b"kind" {
                                            kind = src.string();
                                        } else {
                                            src.skip();
                                        }
                                    }
                                    match kinds.iter_mut().find(|(k, _)| *k == kind) {
                                        Some(e) => e.1 += 1,
                                        None => kinds.push((kind, 1)),
                                    }
                                }
                            }
                            _ => src.skip(),
                        }
                    }
                    if !kinds.is_empty() {
                        for (k, c) in &kinds {
                            *kind_totals.entry(k.clone()).or_default() += c;
                        }
                        frontier_rows.push((id, kinds));
                    }
                    if record_count % 4_000_000 == 0 {
                        eprintln!("records {} ({:.1}s)", record_count, t0.elapsed().as_secs_f64());
                    }
                }
                eprintln!("records {} frontier records {} ({:.1}s)", record_count, frontier_rows.len(), t0.elapsed().as_secs_f64());
            }
            _ => src.skip(),
        }
    }
    let parse_seconds = t0.elapsed().as_secs_f64();
    let total = doms.len();
    assert_eq!(flags.len(), total, "nodes vs domains");

    let key_of = |d: &Dom| -> u32 {
        if d.phase == 0 {
            match owner_of.get(&d.mask) {
                Some(&o) => o,
                None => 100000 + d.mask as u32,
            }
        } else {
            200000 + d.mask as u32
        }
    };
    let keys: Vec<u32> = doms.iter().map(key_of).collect();
    let key_mask = |k: u32| -> String {
        if k < 100000 { mask_str(owner_masks[k as usize]) } else { mask_str((k % 100000) as u16) }
    };
    // ---- frontier rows
    let mut w = std::io::BufWriter::new(File::create(format!("{out}/frontiers.tsv")).unwrap());
    writeln!(w, "id\tphase\towner\tkey\trank\tmax_positive_power\tlower_sum_active\thas_finite_upper\tkinds").unwrap();
    let mut seeds: Vec<u32> = Vec::new();
    for (id, kinds) in &frontier_rows {
        let d = doms[*id as usize];
        let ks: Vec<String> = kinds.iter().map(|(k, c)| format!("{}:{}", String::from_utf8_lossy(k), c)).collect();
        writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", id, if d.phase == 0 { "Apply" } else { "Route" }, mask_str(d.mask), keys[*id as usize],
            if d.rank == 255 { "null".to_string() } else { d.rank.to_string() },
            if d.maxpos == 255 { "null".to_string() } else { d.maxpos.to_string() },
            d.lower_sum_active, d.has_finite_upper as u8, ks.join(",")).unwrap();
        if kinds.iter().any(|(k, _)| k == b"local_dispatch_frontier") {
            seeds.push(*id as u32);
        }
    }
    // ---- owner-level transitions (parallel per edge part)
    let trans: HashMap<(u32, u32), u64> = std::thread::scope(|sc| {
        let hs: Vec<_> = edge_parts.iter().map(|(s, t)| { let keys = &keys; sc.spawn(move || {
            let mut m: FxMap<u64, u64> = FxMap::default();
            for e in 0..s.len() {
                let k = ((keys[s[e] as usize] as u64) << 32) | keys[t[e] as usize] as u64;
                *m.entry(k).or_default() += 1;
            }
            m
        })}).collect();
        let mut all: HashMap<(u32, u32), u64> = HashMap::new();
        for h in hs {
            for (k, v) in h.join().unwrap() {
                *all.entry(((k >> 32) as u32, k as u32)).or_default() += v;
            }
        }
        all
    });
    let mut w = std::io::BufWriter::new(File::create(format!("{out}/transitions.tsv")).unwrap());
    writeln!(w, "source\ttarget\tcount").unwrap();
    let mut tk: Vec<_> = trans.iter().collect();
    tk.sort();
    for (k, v) in tk {
        writeln!(w, "{}\t{}\t{}", k.0, k.1, v).unwrap();
    }
    eprintln!("transitions {} ({:.1}s)", trans.len(), t0.elapsed().as_secs_f64());
    // ---- reverse CSR by target
    let mut off: Vec<u64> = vec![0; total + 1];
    for (_, t) in &edge_parts {
        for &x in t {
            off[x as usize + 1] += 1;
        }
    }
    for i in 0..total {
        off[i + 1] += off[i];
    }
    let mut rsrc: Vec<u32> = vec![0; edge_count];
    {
        let mut fill: Vec<u64> = off[..total].to_vec();
        for (s, t) in &edge_parts {
            for e in 0..s.len() {
                let x = t[e] as usize;
                rsrc[fill[x] as usize] = s[e];
                fill[x] += 1;
            }
        }
    }
    drop(edge_parts);
    eprintln!("reverse CSR built ({:.1}s)", t0.elapsed().as_secs_f64());
    let reverse_walk = |start: &[u32]| -> Vec<bool> {
        let mut seen = vec![false; total];
        let mut stack: Vec<u32> = Vec::new();
        for &s in start {
            if !seen[s as usize] {
                seen[s as usize] = true;
                stack.push(s);
            }
        }
        while let Some(v) = stack.pop() {
            let v = v as usize;
            for e in off[v]..off[v + 1] {
                let u = rsrc[e as usize] as usize;
                if !seen[u] {
                    seen[u] = true;
                    stack.push(u as u32);
                }
            }
        }
        seen
    };
    let anc = reverse_walk(&seeds);
    let mut per_key: HashMap<u32, [u64; 4]> = HashMap::new();
    let mut anc_total = 0u64;
    for v in 0..total {
        if anc[v] {
            anc_total += 1;
            let e = per_key.entry(keys[v]).or_default();
            e[0] += 1;
            e[1] += ((flags[v] >> 1) & 1) as u64;
            e[2] += (flags[v] & 1) as u64;
            e[3] += ((flags[v] >> 2) & 1) as u64;
        }
    }
    drop(anc);
    let mut w = std::io::BufWriter::new(File::create(format!("{out}/ancestors.tsv")).unwrap());
    writeln!(w, "key\tmask\tancestor_nodes\tinspected\tsealed\tclosed").unwrap();
    let mut kk: Vec<_> = per_key.iter().collect();
    kk.sort();
    for (k, v) in kk {
        writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}", k, key_mask(*k), v[0], v[1], v[2], v[3]).unwrap();
    }
    // per seed key: which Apply owner keys are ancestors
    let mut by_seed_key: HashMap<u32, Vec<u32>> = HashMap::new();
    for &s in &seeds {
        by_seed_key.entry(keys[s as usize]).or_default().push(s);
    }
    let mut w = std::io::BufWriter::new(File::create(format!("{out}/seed_ancestors.tsv")).unwrap());
    writeln!(w, "seed_key\tseed_mask\tseeds\tancestor_key\tancestor_mask\tancestor_apply_nodes").unwrap();
    let mut sk: Vec<_> = by_seed_key.keys().copied().collect();
    sk.sort();
    for k in sk {
        let seen = reverse_walk(&by_seed_key[&k]);
        let mut c: HashMap<u32, u64> = HashMap::new();
        for v in 0..total {
            if seen[v] && keys[v] < 200000 {
                *c.entry(keys[v]).or_default() += 1;
            }
        }
        let mut ck: Vec<_> = c.into_iter().collect();
        ck.sort();
        for (a, n) in ck {
            writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}", k, key_mask(k), by_seed_key[&k].len(), a, key_mask(a), n).unwrap();
        }
    }
    // ---- per-key Apply census
    let mut w = std::io::BufWriter::new(File::create(format!("{out}/apply_census.tsv")).unwrap());
    writeln!(w, "key\tmask\tapply_domains\tunbounded_A\tmax_finite_maxpos\tmax_rank\tunbounded_rank\tinspected").unwrap();
    let mut census: HashMap<u32, [u64; 6]> = HashMap::new();
    for v in 0..total {
        let d = doms[v];
        if d.phase != 0 {
            continue;
        }
        let e = census.entry(keys[v]).or_default();
        e[0] += 1;
        if d.maxpos == 255 {
            e[1] += 1;
        } else {
            e[2] = e[2].max(d.maxpos as u64);
        }
        if d.rank == 255 {
            e[4] += 1;
        } else {
            e[3] = e[3].max(d.rank as u64);
        }
        e[5] += ((flags[v] >> 1) & 1) as u64;
    }
    let mut kk: Vec<_> = census.iter().collect();
    kk.sort();
    for (k, v) in kk {
        writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", k, key_mask(*k), v[0], v[1], v[2], v[3], v[4], v[5]).unwrap();
    }
    let mut kinds: Vec<_> = kind_totals.iter().map(|(k, c)| format!("\"{}\":{}", String::from_utf8_lossy(k), c)).collect();
    kinds.sort();
    let mut w = std::io::BufWriter::new(File::create(format!("{out}/summary.json")).unwrap());
    writeln!(w, "{{\"state_file\":\"{}\",\"file_bytes\":{},\"frames\":{},\"edge_array_deframed\":[{},{}],\"threads\":{},\"domains\":{},\"edges\":{},\"records\":{},\"frontier_records\":{},\"local_dispatch_frontier_nodes\":{},\"frontier_kinds\":{{{}}},\"ancestor_nodes\":{},\"parse_seconds\":{:.1},\"total_seconds\":{:.1}}}",
        state, size, nframes, edges_start, edges_end, threads, total, edge_count, record_count, frontier_rows.len(), seeds.len(), kinds.join(","), anc_total, parse_seconds, t0.elapsed().as_secs_f64()).unwrap();
    eprintln!("done ({:.1}s)", t0.elapsed().as_secs_f64());
}
