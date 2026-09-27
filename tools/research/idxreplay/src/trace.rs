//! Reader for the research-only `admission-trace` files written by
//! `crates/rustred-app/src/application/routed_campaign/walking/trace.rs`.
use crate::ckpt::{N, Raw};
use std::fs;
use std::path::Path;

/// Image bytes of a trace written at arity `a` (<= 15; narrower arities are
/// padded with fixed-zero inactive axes, which is exact for projection,
/// words and containment).
pub fn image_bytes(a: usize) -> usize {
    2 + 4 + 4 + 4 * a + 24
}
pub fn coord_record(a: usize) -> usize {
    4 + 4 + 4 + 4 + 8 + 4 + image_bytes(a)
}

pub const EXACT: u8 = 0;
pub const ORTHANT: u8 = 1;
pub const CONTAINED: u8 = 2;
pub const NEW: u8 = 3;
pub const REFUSED: u8 = 4;

fn u32_at(b: &[u8], p: usize) -> u32 {
    u32::from_le_bytes(b[p..p + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], p: usize) -> u64 {
    u64::from_le_bytes(b[p..p + 8].try_into().unwrap())
}

/// `CompactDomain::trace_image` -> raw domain.
pub fn image(b: &[u8], a: usize) -> Raw {
    let phase = b[0];
    let flags = b[1];
    let owner = u32_at(b, 2);
    let rank = u32_at(b, 6);
    let mut lower = [0u16; N];
    let mut upper = [0u16; N];
    let mut p = 10;
    for l in lower.iter_mut().take(a) {
        *l = u16::from_le_bytes([b[p], b[p + 1]]);
        p += 2;
    }
    for u in upper.iter_mut().take(a) {
        *u = u16::from_le_bytes([b[p], b[p + 1]]);
        p += 2;
    }
    let amax = u64_at(b, p);
    let dmin = u64_at(b, p + 8) as i64;
    let dmax = u64_at(b, p + 16) as i64;
    assert!(owner < 1 << N);
    Raw {
        phase,
        owner: owner as u16,
        lower,
        upper,
        rank: (flags & 1 == 0).then_some(rank),
        amax: (flags & 2 == 0).then_some(amax),
        dmin: (flags & 4 == 0).then_some(dmin),
        dmax: (flags & 8 == 0).then_some(dmax),
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Admission {
    pub kind: u8,
    pub source: u32,
    pub target: u32,
    pub forward: u32,
    pub maintenance: u64,
    pub retired: u32,
    pub raw: Raw,
}

fn check_header(b: &[u8], magic: &[u8; 8]) -> usize {
    assert_eq!(&b[..8], magic, "trace magic");
    let a = u32_at(b, 8) as usize;
    assert!((1..=N).contains(&a), "trace arity {a} > {N}");
    a
}

pub fn read_coord(path: &Path) -> Vec<Admission> {
    read_coord_arity(path).1
}

pub fn read_coord_arity(path: &Path) -> (usize, Vec<Admission>) {
    let b = fs::read(path).unwrap();
    let a = check_header(&b, b"RRTRCRD1");
    let rec = coord_record(a);
    let body = &b[12..];
    let whole = body.len() / rec;
    if body.len() % rec != 0 {
        eprintln!("{}: {} trailing bytes (truncated record ignored)", path.display(), body.len() % rec);
    }
    let v = (0..whole)
        .map(|k| {
            let r = &body[k * rec..(k + 1) * rec];
            Admission {
                kind: r[0],
                source: u32_at(r, 4),
                target: u32_at(r, 8),
                forward: u32_at(r, 12),
                maintenance: u64_at(r, 16),
                retired: u32_at(r, 24),
                raw: image(&r[28..], a),
            }
        })
        .collect();
    (a, v)
}

#[derive(Clone, Debug)]
pub enum Ev {
    Admit { raw: Raw, successor: bool, count: u32 },
    KnownReuse { count: u32 },
    PreAdmitted { target: u32, count: u32 },
    Frontier { count: u32 },
    Unrepresentable { count: u32 },
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: u32,
    pub total: u64,
    pub seconds: f64,
    pub error: bool,
    pub partial: bool,
    pub stopped: bool,
    pub route: bool,
    pub parent: Raw,
    pub events: Vec<Ev>,
    pub thread_file: u32,
    pub seq: u32,
}

pub fn read_jobs(path: &Path, file_index: u32) -> Vec<Job> {
    let b = fs::read(path).unwrap();
    let a = check_header(&b, b"RRTRJOB1");
    let ib = image_bytes(a);
    let mut p = 12;
    let mut out = Vec::new();
    let header = 4 + 4 + 4 + 4 + 8 + 8 + 4 + ib;
    while p + header <= b.len() {
        assert_eq!(u32_at(&b, p), 0x4A4F_4231, "job magic");
        let id = u32_at(&b, p + 4);
        let len = u32_at(&b, p + 8) as usize;
        let records = u32_at(&b, p + 12) as usize;
        let total = u64_at(&b, p + 16);
        let seconds = f64::from_le_bytes(b[p + 24..p + 32].try_into().unwrap());
        let status = b[p + 32];
        let parent = image(&b[p + 36..p + 36 + ib], a);
        let body_start = p + header;
        if body_start + len > b.len() {
            eprintln!("{}: truncated job at byte {p}", path.display());
            break;
        }
        let body = &b[body_start..body_start + len];
        let mut q = 0;
        let mut events = Vec::with_capacity(records);
        while q < body.len() {
            let tag = body[q];
            let flags = body[q + 1];
            let count = u32_at(body, q + 2);
            q += 6;
            events.push(match tag {
                1 => {
                    let raw = image(&body[q..q + ib], a);
                    q += ib;
                    Ev::Admit {
                        raw,
                        successor: flags & 1 != 0,
                        count,
                    }
                }
                2 => Ev::KnownReuse { count },
                3 => {
                    let target = u32_at(body, q);
                    q += 4;
                    Ev::PreAdmitted { target, count }
                }
                4 => Ev::Frontier { count },
                9 => Ev::Unrepresentable { count },
                x => panic!("event tag {x}"),
            });
        }
        assert_eq!(events.len(), records, "job record count");
        out.push(Job {
            id,
            total,
            seconds,
            error: status & 1 != 0,
            partial: status & 2 != 0,
            stopped: status & 4 != 0,
            route: status & 8 != 0,
            parent,
            events,
            thread_file: file_index,
            seq: out.len() as u32,
        });
        p = body_start + len;
    }
    out
}

pub fn files(dir: &Path, prefix: &str) -> Vec<std::path::PathBuf> {
    let mut v: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with(prefix))
        .collect();
    v.sort();
    v
}
