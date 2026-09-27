//! Read-only decoders of a RUSTRED-WALK-CP5 checkpoint directory
//! (`walking/checkpoint/sections.rs`): 32-byte section headers, bincode-2
//! standard (varint, zigzag for signed) domain records, the stored ledger,
//! the index image (for the live-candidate bitmap only), node flags and
//! (u32 source, u32 target) edges. Nothing is ever written back.
use crate::geom::{DLO_NONE, DHI_NONE, Dom, INF8, MAXN, NONE16};
use rayon::prelude::*;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub const HEADER: usize = 32;

pub struct R<'a> {
    pub b: &'a [u8],
    pub p: usize,
}
impl<'a> R<'a> {
    #[inline]
    pub fn u8(&mut self) -> u8 {
        let v = self.b[self.p];
        self.p += 1;
        v
    }
    #[inline]
    pub fn var(&mut self) -> u128 {
        let t = self.u8();
        match t {
            0..=250 => t as u128,
            251 => {
                let v = u16::from_le_bytes(self.b[self.p..self.p + 2].try_into().unwrap());
                self.p += 2;
                v as u128
            }
            252 => {
                let v = u32::from_le_bytes(self.b[self.p..self.p + 4].try_into().unwrap());
                self.p += 4;
                v as u128
            }
            253 => {
                let v = u64::from_le_bytes(self.b[self.p..self.p + 8].try_into().unwrap());
                self.p += 8;
                v as u128
            }
            254 => {
                let v = u128::from_le_bytes(self.b[self.p..self.p + 16].try_into().unwrap());
                self.p += 16;
                v
            }
            _ => panic!("bad varint tag {t} at {}", self.p),
        }
    }
    #[inline]
    pub fn ivar(&mut self) -> i128 {
        let u = self.var();
        ((u >> 1) as i128) ^ -((u & 1) as i128)
    }
    #[inline]
    pub fn opt(&mut self) -> Option<u128> {
        match self.u8() {
            0 => None,
            1 => Some(self.var()),
            x => panic!("bad option tag {x} at {}", self.p),
        }
    }
    #[inline]
    pub fn iopt(&mut self) -> Option<i128> {
        match self.u8() {
            0 => None,
            1 => Some(self.ivar()),
            x => panic!("bad option tag {x} at {}", self.p),
        }
    }
}

pub struct Header {
    pub tag: [u8; 4],
    pub arity: usize,
    pub count: usize,
    pub first: usize,
}
pub fn header(b: &[u8]) -> Header {
    assert_eq!(&b[0..4], b"RRW5", "section magic");
    Header {
        tag: b[4..8].try_into().unwrap(),
        arity: u16::from_le_bytes(b[8..10].try_into().unwrap()) as usize,
        count: u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize,
        first: u64::from_le_bytes(b[24..32].try_into().unwrap()) as usize,
    }
}

pub fn map(path: &Path) -> memmap2::Mmap {
    let f = std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    unsafe { memmap2::Mmap::map(&f).unwrap() }
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub generation: u64,
    pub path: PathBuf,
    pub first: usize,
    pub count: usize,
}

pub struct Manifest {
    pub dir: PathBuf,
    pub generation: u64,
    pub arity: usize,
    pub policy: String,
    pub nodes: PathBuf,
    pub ledger: Option<PathBuf>,
    pub index: PathBuf,
    pub meta: PathBuf,
    pub domains: Vec<Segment>,
    pub edges: Vec<Segment>,
    pub records: Vec<Segment>,
    pub raw: Value,
}

fn segments(dir: &Path, v: &Value) -> Vec<Segment> {
    let mut out: Vec<Segment> = v["segments"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|s| Segment {
                    generation: s["generation"].as_u64().unwrap(),
                    path: dir.join(s["file"].as_str().unwrap()),
                    first: s["first"].as_u64().unwrap() as usize,
                    count: s["count"].as_u64().unwrap() as usize,
                })
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|s| s.first);
    out
}

/// `latest.json` of the directory, or `manifest` if given (a previous.json).
pub fn manifest(dir: &Path, name: Option<&str>) -> Manifest {
    let file = dir.join(name.unwrap_or("latest.json"));
    let raw: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(raw["format"], "RUSTRED-WALK-CP5", "{}", file.display());
    let s = &raw["sections"];
    let f = |k: &str| dir.join(s[k]["file"].as_str().unwrap_or_else(|| panic!("section {k}")));
    Manifest {
        dir: dir.to_path_buf(),
        generation: raw["generation"].as_u64().unwrap(),
        arity: raw["arity"].as_u64().unwrap() as usize,
        policy: raw["publication_policy"].as_str().unwrap_or("").to_string(),
        nodes: f("nodes"),
        ledger: s.get("ledger").and_then(|l| l["file"].as_str()).map(|x| dir.join(x)),
        index: f("index"),
        meta: f("meta"),
        domains: segments(dir, &s["domains"]),
        edges: segments(dir, &s["edges"]),
        records: segments(dir, &s["records"]),
        raw: raw.clone(),
    }
}

/// All domains in ID order.
pub fn domains(m: &Manifest) -> Vec<Dom> {
    let n = m.arity;
    assert!(n <= MAXN);
    let parts: Vec<(usize, Vec<Dom>)> = m
        .domains
        .par_iter()
        .map(|seg| {
            let b = map(&seg.path);
            let h = header(&b);
            assert_eq!(&h.tag, b"DOMS");
            assert_eq!(h.arity, n);
            assert_eq!((h.first, h.count), (seg.first, seg.count));
            let mut r = R { b: &b, p: HEADER };
            let mut out = Vec::with_capacity(h.count);
            for _ in 0..h.count {
                let mut d = Dom::default();
                d.phase = r.var() as u8;
                assert!(d.phase <= 1);
                let ol = r.var() as usize;
                assert_eq!(ol, n);
                for i in 0..n {
                    if r.u8() == 1 {
                        d.owner |= 1 << i;
                    }
                }
                let ll = r.var() as usize;
                assert_eq!(ll, n);
                for i in 0..n {
                    let v = r.var();
                    assert!(v < INF8 as u128, "lower coordinate {v}");
                    d.lo[i] = v as u8;
                }
                let ul = r.var() as usize;
                assert_eq!(ul, n);
                for i in 0..n {
                    d.hi[i] = match r.opt() {
                        None => INF8,
                        Some(v) => {
                            assert!(v < INF8 as u128, "upper coordinate {v}");
                            v as u8
                        }
                    };
                }
                d.rank = match r.opt() {
                    None => INF8,
                    Some(v) => {
                        assert!(v < INF8 as u128, "rank {v}");
                        v as u8
                    }
                };
                d.amax = match r.opt() {
                    None => NONE16,
                    Some(v) => {
                        assert!(v < NONE16 as u128);
                        v as u16
                    }
                };
                d.dmin = match r.iopt() {
                    None => DLO_NONE,
                    Some(v) => {
                        assert!(v > DLO_NONE as i128 && v < DHI_NONE as i128);
                        v as i16
                    }
                };
                d.dmax = match r.iopt() {
                    None => DHI_NONE,
                    Some(v) => {
                        assert!(v > DLO_NONE as i128 && v < DHI_NONE as i128);
                        v as i16
                    }
                };
                out.push(d);
            }
            assert_eq!(r.p, b.len(), "{} trailing bytes", seg.path.display());
            (seg.first, out)
        })
        .collect();
    let mut all = Vec::new();
    for (first, v) in parts {
        assert_eq!(first, all.len(), "domain segments not contiguous");
        all.extend(v);
    }
    all
}

/// Node flags: bit0 sealed, bit1 inspected, bit2 closed.
pub fn nodes(m: &Manifest) -> Vec<u8> {
    let b = map(&m.nodes);
    let h = header(&b);
    assert_eq!(&h.tag, b"NODE");
    assert_eq!(b.len(), HEADER + h.count);
    b[HEADER..].to_vec()
}

/// Ledger entry state.
pub const L_UNRESERVED: u8 = 0;
pub const L_RESERVED: u8 = 1;
pub const L_STARTED: u8 = 2;
pub const L_COMPLETED: u8 = 3;
pub const L_FAILED: u8 = 4;
pub const L_CANCELLED: u8 = 5;
pub const L_DELEGATE: u8 = 6;

#[derive(Clone, Copy, Default)]
pub struct LEntry {
    pub state: u8,
    pub delegated_published: u8,
    /// Delegate target, or unresolved frontiers for Completed.
    pub to: u32,
    /// initial anchor id + 1 (0 = none).
    pub anchor: u32,
}

pub struct Ledger {
    pub ready: bool,
    pub entries: Vec<LEntry>,
    pub cursor: usize,
    pub lookahead: usize,
    pub reserved_through: usize,
    pub transfers: usize,
    pub native_publications: usize,
    pub delegated_publications: usize,
    pub protected_initial_prefix: Option<usize>,
}

/// `delegation/ledger/checkpoint.rs` StoredLedger (bincode-2 standard).
pub fn ledger(m: &Manifest) -> Option<Ledger> {
    let path = m.ledger.as_ref()?;
    let b = map(path);
    let h = header(&b);
    assert_eq!(&h.tag, b"LEDG");
    let mut r = R { b: &b, p: HEADER };
    let ready = r.u8() == 1;
    let _outstanding = r.opt();
    let n = r.var() as usize;
    let mut entries = Vec::with_capacity(n);
    for _ in 0..n {
        let mut e = LEntry::default();
        match r.var() {
            0 => match r.var() {
                0 => e.state = L_UNRESERVED,
                1 => e.state = L_RESERVED,
                2 => e.state = L_STARTED,
                3 => match r.var() {
                    0 => {
                        e.state = L_COMPLETED;
                        e.to = r.var() as u32;
                    }
                    1 => e.state = L_FAILED,
                    2 => e.state = L_CANCELLED,
                    x => panic!("native outcome {x}"),
                },
                x => panic!("local {x}"),
            },
            1 => {
                e.state = L_DELEGATE;
                e.to = r.var() as u32;
            }
            x => panic!("responsibility {x} at {}", r.p),
        }
        if let Some(a) = r.opt() {
            e.anchor = a as u32 + 1;
        }
        if let Some(p) = r.opt() {
            e.delegated_published = p as u8;
        }
        entries.push(e);
    }
    let cursor = r.var() as usize;
    let lookahead = r.var() as usize;
    let reserved_through = r.var() as usize;
    let _max_domains = r.var();
    let transfers = r.var() as usize;
    let native_publications = r.var() as usize;
    let delegated_publications = r.var() as usize;
    let _halted = r.u8();
    let _initial_admission = r.u8();
    let protected_initial_prefix = r.opt().map(|v| v as usize);
    let _partial = r.var();
    assert_eq!(r.p, b.len(), "ledger trailing bytes");
    assert_eq!(n, h.count);
    Some(Ledger {
        ready,
        entries,
        cursor,
        lookahead,
        reserved_through,
        transfers,
        native_publications,
        delegated_publications,
        protected_initial_prefix,
    })
}

/// Live-candidate bitmap from the index image (port of the lens tool
/// `indexscan`), plus the number of buckets.
pub fn live(m: &Manifest, n: usize) -> (Vec<bool>, usize) {
    let idx = map(&m.index);
    let h = header(&idx);
    assert_eq!(&h.tag, b"INDX");
    let mut live = vec![false; n];
    let mut r = R { b: &idx, p: HEADER };
    let buckets = r.var() as usize;
    for _ in 0..buckets {
        r.var();
        let olen = r.var() as usize;
        r.p += olen;
        let nids = r.var() as usize;
        for _ in 0..nids {
            r.var();
        }
        let ng = r.var() as usize;
        for _ in 0..ng {
            match r.var() {
                0 => {}
                1 => {
                    for _ in 0..2 {
                        if r.var() == 0 {
                            r.var();
                        }
                    }
                    if r.var() == 1 {
                        r.var();
                    }
                }
                x => panic!("signature {x}"),
            }
            let nb = r.var() as usize;
            for _ in 0..nb {
                let mut ids = [0usize; 32];
                for id in ids.iter_mut() {
                    *id = r.var() as usize;
                }
                let len = r.var() as usize;
                let env = r.var() as usize;
                for _ in 0..env {
                    r.var();
                    r.var();
                    r.opt();
                    r.opt();
                }
                for &id in &ids[..len] {
                    if id < n {
                        live[id] = true;
                    }
                }
            }
            r.var();
        }
        r.var();
        r.opt();
    }
    assert_eq!(r.p, idx.len(), "index trailing bytes");
    (live, buckets)
}

/// Visit every edge segment in parallel chunks: f(first_edge_index, &[(s,t)]).
pub fn edges_par<F: Fn(usize, &[u8]) + Sync>(m: &Manifest, chunk_edges: usize, f: F) {
    for seg in &m.edges {
        let b = map(&seg.path);
        let h = header(&b);
        assert_eq!(&h.tag, b"EDGE");
        assert_eq!(b.len(), HEADER + 8 * h.count);
        let payload = &b[HEADER..];
        payload
            .par_chunks(8 * chunk_edges)
            .enumerate()
            .for_each(|(k, c)| f(seg.first + k * chunk_edges, c));
    }
}

#[inline]
pub fn edge_at(c: &[u8], i: usize) -> (u32, u32) {
    (
        u32::from_le_bytes(c[8 * i..8 * i + 4].try_into().unwrap()),
        u32::from_le_bytes(c[8 * i + 4..8 * i + 8].try_into().unwrap()),
    )
}

impl Manifest {
    /// Placeholder for inputs that are not CP5 directories.
    pub fn dummy(path: &Path, arity: usize) -> Self {
        Manifest {
            dir: path.to_path_buf(),
            generation: 0,
            arity,
            policy: String::new(),
            nodes: PathBuf::new(),
            ledger: None,
            index: PathBuf::new(),
            meta: PathBuf::new(),
            domains: Vec::new(),
            edges: Vec::new(),
            records: Vec::new(),
            raw: Value::Null,
        }
    }
}
