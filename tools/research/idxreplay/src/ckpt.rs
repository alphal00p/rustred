//! Read-only decoding of a CP5 walk checkpoint (v2 campaign, arity 15) and the
//! exact tight projection of `rustred-core` `power_domain/geometry.rs::project`.
//!
//! Section layout (`walking/checkpoint/sections.rs`): 32-byte header (magic
//! RRW5, tag, arity, flags, semantics, count u64 @16, first u64 @24), then
//! bincode-standard varint payloads (domains, index) or fixed records
//! (nodes: one flag byte per ID; edges: (u32 source, u32 target)).
use std::fs;
use std::path::Path;

pub const N: usize = 15;
pub const U16INF: u16 = u16::MAX;
pub const UINF: u32 = u32::MAX;
pub const NEG: i32 = i32::MIN;
pub const POS: i32 = i32::MAX;

pub const FLAG_SEALED: u8 = 1;
pub const FLAG_INSPECTED: u8 = 2;

pub struct R<'a> {
    pub b: &'a [u8],
    pub p: usize,
}
impl<'a> R<'a> {
    pub fn u8(&mut self) -> u8 {
        let v = self.b[self.p];
        self.p += 1;
        v
    }
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
            _ => panic!("bad varint {} at {}", t, self.p),
        }
    }
    pub fn opt(&mut self) -> Option<u128> {
        match self.u8() {
            0 => None,
            1 => Some(self.var()),
            x => panic!("opt tag {x} at {}", self.p),
        }
    }
}
pub fn zz(v: u128) -> i64 {
    let v = v as u64;
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

/// A raw domain as admitted (the `CompactDomain` fields).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Raw {
    pub phase: u8,
    pub owner: u16,
    pub lower: [u16; N],
    /// U16INF = +infinity.
    pub upper: [u16; N],
    pub rank: Option<u32>,
    pub amax: Option<u64>,
    pub dmin: Option<i64>,
    pub dmax: Option<i64>,
}

/// Tight native extrema (`DomainPowerSummary` / `CompactSummary`), narrowed:
/// coordinates u16 (U16INF = +inf), A/R u32 (UINF = +inf), D i32 (NEG/POS).
/// `phase`/`owner` are carried for bucket keys only; containment is always
/// evaluated inside one (phase, owner) bucket, as in the queue.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(C)]
pub struct T {
    pub lo: [u16; N],
    pub up: [u16; N],
    pub pl: u32,
    pub pu: u32,
    pub nl: u32,
    pub nu: u32,
    pub dl: i32,
    pub du: i32,
    pub owner: u16,
    pub phase: u8,
    pub empty: bool,
}

pub static CLAMPS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn clamp_note() {
    CLAMPS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
}

fn mn(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
fn mx(a: Option<i128>, b: Option<i128>) -> Option<i128> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}
fn cu(v: Option<i128>) -> u32 {
    v.map_or(UINF, |x| {
        if x > (UINF - 1) as i128 {
            clamp_note();
        }
        x.clamp(0, (UINF - 1) as i128) as u32
    })
}
fn c16(x: i128) -> u16 {
    if x > 65534 {
        clamp_note();
    }
    x.clamp(0, 65534) as u16
}
fn ci(v: Option<i128>, inf: i32) -> i32 {
    v.map_or(inf, |x| {
        if x <= i32::MIN as i128 || x >= i32::MAX as i128 {
            clamp_note();
        }
        x.clamp(i32::MIN as i128 + 1, i32::MAX as i128 - 1) as i32
    })
}

impl T {
    pub const EMPTY: T = T {
        lo: [0; N],
        up: [0; N],
        pl: 0,
        pu: 0,
        nl: 0,
        nu: 0,
        dl: 0,
        du: 0,
        owner: 0,
        phase: 0,
        empty: true,
    };

    /// Port of `project` (checked arithmetic cannot overflow at these widths).
    pub fn project(d: &Raw) -> T {
        let own = |ax: usize| (d.owner >> ax) & 1 == 1;
        let mut gl = [0i128; 2];
        let mut gu = [0i128; 2];
        let mut gun = [0usize; 2];
        let mut active = 0i128;
        let up = |ax: usize| (d.upper[ax] != U16INF).then_some(d.upper[ax] as i128);
        for ax in 0..N {
            let g = own(ax) as usize;
            gl[g] += d.lower[ax] as i128;
            match up(ax) {
                Some(u) => gu[g] += u,
                None => gun[g] += 1,
            }
            active += own(ax) as i128;
        }
        let gup = |g: usize| if gun[g] == 0 { Some(gu[g]) } else { None };
        let mut empty = T::EMPTY;
        empty.owner = d.owner;
        empty.phase = d.phase;
        let a_lo = gl[1] + active;
        let a_up = mn(gup(1).map(|v| v + active), d.amax.map(|v| v as i128));
        let r_lo = gl[0];
        let r_up = mn(gup(0), d.rank.map(|v| v as i128));
        if a_up.is_some_and(|u| u < a_lo) || r_up.is_some_and(|u| u < r_lo) {
            return empty;
        }
        let d_lo = mx(d.dmin.map(|v| v as i128), r_up.map(|u| a_lo - u));
        let d_up = mn(d.dmax.map(|v| v as i128), a_up.map(|u| u - r_lo));
        if let (Some(l), Some(u)) = (d_lo, d_up)
            && l > u
        {
            return empty;
        }
        let pa_lo = d_lo.map_or(a_lo, |dd| a_lo.max(r_lo + dd));
        let pa_up = mn(a_up, r_up.zip(d_up).map(|(r, dd)| r + dd));
        let pr_lo = d_up.map_or(r_lo, |dd| r_lo.max(a_lo - dd));
        let pr_up = mn(r_up, a_up.zip(d_lo).map(|(a, dd)| a - dd));
        if pa_up.is_some_and(|u| u < pa_lo) || pr_up.is_some_and(|u| u < pr_lo) {
            return empty;
        }
        let la = (pa_lo - active, pa_up.map(|u| u - active));
        let iv = [(pr_lo, pr_up), la];
        let mut lo = [0u16; N];
        let mut upo = [U16INF; N];
        for ax in 0..N {
            let g = own(ax) as usize;
            let (il, iu) = iv[g];
            let lower = d.lower[ax] as i128;
            let other_lower = gl[g] - lower;
            let rem_unb = gun[g] - up(ax).is_none() as usize;
            let other_upper = if rem_unb != 0 {
                None
            } else {
                Some(up(ax).map_or(gu[g], |u| gu[g] - u))
            };
            let l = other_upper.map_or(lower, |o| lower.max(il - o));
            let h = mn(up(ax), iu.map(|u| u - other_lower));
            lo[ax] = c16(l);
            upo[ax] = h.map_or(U16INF, c16);
        }
        T {
            lo,
            up: upo,
            pl: cu(Some(pa_lo)),
            pu: cu(pa_up),
            nl: cu(Some(pr_lo)),
            nu: cu(pr_up),
            dl: ci(d_lo, NEG),
            du: ci(d_up, POS),
            owner: d.owner,
            phase: d.phase,
            empty: false,
        }
    }

    /// `DomainPowerSummary::contains` within one (phase, owner) bucket.
    #[inline]
    pub fn contains(&self, q: &T) -> bool {
        if q.empty {
            return true;
        }
        if self.empty {
            return false;
        }
        let upc = |a: u32, b: u32| a == UINF || (b != UINF && b <= a);
        let upc16 = |a: u16, b: u16| a == U16INF || (b != U16INF && b <= a);
        self.pl <= q.pl
            && upc(self.pu, q.pu)
            && self.nl <= q.nl
            && upc(self.nu, q.nu)
            && (self.dl == NEG || (q.dl != NEG && q.dl >= self.dl))
            && (self.du == POS || (q.du != POS && q.du <= self.du))
            && (0..N).all(|i| self.lo[i] <= q.lo[i])
            && (0..N).all(|i| upc16(self.up[i], q.up[i]))
    }

    /// `bits::word`: the packed necessary-condition word (bits.rs layout).
    pub fn word(&self) -> u64 {
        if self.empty {
            return 0;
        }
        let mut w = 0u64;
        for ax in 0..N {
            w |= u64::from(self.up[ax] == U16INF) << ax;
            w |= u64::from(self.lo[ax] == 0) << (16 + ax);
        }
        w |= u64::from(self.pu == UINF) << 32;
        w |= u64::from(self.nu == UINF) << 33;
        w |= u64::from(self.dl == NEG) << 34;
        w |= u64::from(self.du == POS) << 35;
        w |= u64::from(self.pl == 0) << 36;
        w |= u64::from(self.nl == 0) << 37;
        w |= u64::from(self.dl == NEG || self.dl <= 0) << 38;
        w
    }

    pub fn finite_pattern(&self) -> u16 {
        (0..N).map(|i| ((self.up[i] != U16INF) as u16) << i).sum()
    }
}

/// Group signature (`index::Signature`): A upper, R upper, D lower.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Sig {
    pub empty: bool,
    pub pos: u32,
    pub num: u32,
    pub dif: i32,
}
impl Sig {
    pub fn of(t: &T) -> Sig {
        if t.empty {
            Sig {
                empty: true,
                pos: 0,
                num: 0,
                dif: 0,
            }
        } else {
            Sig {
                empty: false,
                pos: t.pu,
                num: t.nu,
                dif: t.dl,
            }
        }
    }
    /// Necessary: Q subset C implies c.may_contain(q).
    #[inline]
    pub fn may_contain(&self, q: &Sig) -> bool {
        if q.empty {
            return true;
        }
        if self.empty {
            return false;
        }
        let upc = |a: u32, b: u32| a == UINF || (b != UINF && b <= a);
        upc(self.pos, q.pos) && upc(self.num, q.num) && (self.dif == NEG || (q.dif != NEG && q.dif >= self.dif))
    }
}

pub fn header(b: &[u8]) -> (u64, u64) {
    assert_eq!(&b[0..4], b"RRW5", "section magic");
    (
        u64::from_le_bytes(b[16..24].try_into().unwrap()),
        u64::from_le_bytes(b[24..32].try_into().unwrap()),
    )
}

pub fn decode_domain(r: &mut R) -> Raw {
    let phase = r.var() as u8;
    let ol = r.var() as usize;
    let mut owner = 0u16;
    for k in 0..ol {
        if r.b[r.p + k] == 1 {
            owner |= 1 << k;
        }
    }
    r.p += ol;
    assert_eq!(r.var() as usize, N);
    let mut lower = [0u16; N];
    for l in lower.iter_mut() {
        *l = u16::try_from(r.var()).expect("compact lower");
    }
    assert_eq!(r.var() as usize, N);
    let mut upper = [U16INF; N];
    for u in upper.iter_mut() {
        if let Some(v) = r.opt() {
            *u = u16::try_from(v).ok().filter(|&v| v != U16INF).expect("compact upper");
        }
    }
    let rank = match r.u8() {
        0 => None,
        1 => Some(r.var() as u32),
        x => panic!("rank tag {x}"),
    };
    let amax = r.opt().map(|v| v as u64);
    let dmin = r.opt().map(zz);
    let dmax = r.opt().map(zz);
    Raw {
        phase,
        owner,
        lower,
        upper,
        rank,
        amax,
        dmin,
        dmax,
    }
}

pub struct Segment {
    pub first: usize,
    pub count: usize,
    pub raws: Vec<Raw>,
}

/// Decode every domain segment (in parallel, one thread per segment).
pub fn load_domains(dir: &Path, max_id: Option<usize>) -> Vec<Segment> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("domains-"))
        .collect();
    paths.sort();
    let mut segs: Vec<Segment> = std::thread::scope(|s| {
        let handles: Vec<_> = paths
            .iter()
            .map(|p| {
                s.spawn(move || {
                    let b = fs::read(p).unwrap();
                    let (count, first) = header(&b);
                    let (count, first) = (count as usize, first as usize);
                    let mut r = R { b: &b, p: 32 };
                    let mut raws = Vec::with_capacity(count);
                    for _ in 0..count {
                        raws.push(decode_domain(&mut r));
                    }
                    assert_eq!(r.p, b.len(), "domain segment trailing bytes");
                    Segment { first, count, raws }
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    segs.sort_by_key(|s| s.first);
    let mut next = 0;
    let mut out = Vec::new();
    for s in segs {
        if max_id.is_some_and(|m| s.first >= m) {
            continue;
        }
        assert_eq!(s.first, next, "domain segments not contiguous");
        next += s.count;
        out.push(s);
    }
    out
}

/// Stored candidate index: buckets -> groups (signature, blocks of IDs with
/// the stored per-axis envelopes), exactly as persisted.
pub struct StoredEnv {
    pub min_lower: u64,
    pub max_lower: u64,
    pub min_upper: Option<u64>,
    pub max_upper: Option<u64>,
}
pub struct StoredBlock {
    pub ids: Vec<u32>,
    pub env: Vec<StoredEnv>,
}
pub struct StoredGroup {
    pub sig: Sig,
    pub blocks: Vec<StoredBlock>,
    pub live: usize,
}
pub struct StoredBucket {
    pub phase: u8,
    pub owner: u16,
    pub groups: Vec<StoredGroup>,
    pub orthant: Option<u32>,
}

pub fn load_index(path: &Path) -> Vec<StoredBucket> {
    let idx = fs::read(path).unwrap();
    let mut r = R { b: &idx, p: 32 };
    let nb = r.var() as usize;
    let mut out = Vec::with_capacity(nb);
    for _ in 0..nb {
        let phase = r.var() as u8;
        let ol = r.var() as usize;
        let mut owner = 0u16;
        for k in 0..ol {
            if r.b[r.p + k] == 1 {
                owner |= 1 << k;
            }
        }
        r.p += ol;
        let nids = r.var() as usize;
        assert_eq!(nids, 0, "finite-cap lane ids present");
        let ng = r.var() as usize;
        let mut groups = Vec::with_capacity(ng);
        for _ in 0..ng {
            let sig = match r.var() {
                0 => Sig {
                    empty: true,
                    pos: 0,
                    num: 0,
                    dif: 0,
                },
                1 => {
                    let p = match r.var() {
                        0 => cu(Some(r.var() as i128)),
                        1 => UINF,
                        x => panic!("{x}"),
                    };
                    let q = match r.var() {
                        0 => cu(Some(r.var() as i128)),
                        1 => UINF,
                        x => panic!("{x}"),
                    };
                    let d = match r.var() {
                        0 => NEG,
                        1 => ci(Some(zz(r.var()) as i128), NEG),
                        x => panic!("{x}"),
                    };
                    Sig {
                        empty: false,
                        pos: p,
                        num: q,
                        dif: d,
                    }
                }
                x => panic!("sig {x}"),
            };
            let nbl = r.var() as usize;
            let mut blocks = Vec::with_capacity(nbl);
            for _ in 0..nbl {
                let mut ids = [0u32; 32];
                for id in ids.iter_mut() {
                    *id = r.var() as u32;
                }
                let len = r.var() as usize;
                let ne = r.var() as usize;
                let mut env = Vec::with_capacity(ne);
                for _ in 0..ne {
                    let min_lower = r.var() as u64;
                    let max_lower = r.var() as u64;
                    let min_upper = r.opt().map(|v| v as u64);
                    let max_upper = r.opt().map(|v| v as u64);
                    env.push(StoredEnv {
                        min_lower,
                        max_lower,
                        min_upper,
                        max_upper,
                    });
                }
                blocks.push(StoredBlock {
                    ids: ids[..len].to_vec(),
                    env,
                });
            }
            let live = r.var() as usize;
            groups.push(StoredGroup { sig, blocks, live });
        }
        let _bucket_live = r.var();
        let orthant = r.opt().map(|v| v as u32);
        out.push(StoredBucket {
            phase,
            owner,
            groups,
            orthant,
        });
    }
    assert_eq!(r.p, idx.len(), "index trailing bytes");
    out
}

pub fn load_nodes(path: &Path) -> Vec<u8> {
    let b = fs::read(path).unwrap();
    let (count, _) = header(&b);
    assert_eq!(b.len(), 32 + count as usize);
    b[32..].to_vec()
}

/// Edge segment paths in order.
pub fn edge_segments(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("edges-"))
        .collect();
    paths.sort();
    paths
}

pub fn owner_str(owner: u16) -> String {
    (0..N).map(|k| if owner >> k & 1 == 1 { '1' } else { '0' }).collect()
}
