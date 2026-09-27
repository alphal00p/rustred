// Read-only replay of admission lookups against a CP5 live index (perf skeptic, v2).
// Exact tight projection (port of rustred-core power_domain/geometry.rs project)
// and exact DomainPowerSummary::contains (same phase+owner assumed by bucket).
// Layouts: (a) stored index (ID-ordered 32-blocks, stored envelopes);
//          (b) clustered 64-blocks per signature group sorted by (sum tight lower, sum finite upper),
//              envelopes recomputed from tight extrema, block min(sum lower) skip.
// Miss proxies: live IDs admitted in the newest segment, scanned with self excluded.
// Hit proxies: delegated / retired-not-inspected IDs (known to have a live container).
use std::fs;

struct R<'a> { b: &'a [u8], p: usize }
impl<'a> R<'a> {
    fn u8(&mut self) -> u8 { let v = self.b[self.p]; self.p += 1; v }
    fn var(&mut self) -> u128 {
        let t = self.u8();
        match t {
            0..=250 => t as u128,
            251 => { let v = u16::from_le_bytes(self.b[self.p..self.p+2].try_into().unwrap()); self.p += 2; v as u128 }
            252 => { let v = u32::from_le_bytes(self.b[self.p..self.p+4].try_into().unwrap()); self.p += 4; v as u128 }
            253 => { let v = u64::from_le_bytes(self.b[self.p..self.p+8].try_into().unwrap()); self.p += 8; v as u128 }
            254 => { let v = u128::from_le_bytes(self.b[self.p..self.p+16].try_into().unwrap()); self.p += 16; v }
            _ => panic!("bad varint {} at {}", t, self.p),
        }
    }
    fn opt(&mut self) -> Option<u128> { match self.u8() { 0 => None, 1 => Some(self.var()), x => panic!("opt {x} at {}", self.p) } }
}
fn zz(v: u128) -> i64 { let v = v as u64; ((v >> 1) as i64) ^ -((v & 1) as i64) }
const N: usize = 15;
const UINF: u32 = u32::MAX;
const NEG: i32 = i32::MIN;
const POS: i32 = i32::MAX;
// tight summary; empty => all fields ignored
#[derive(Clone, Copy)]
struct T { phase: u8, owner: u16, empty: bool, lo: [u16; N], up: [u16; N], pl: u32, pu: u32, nl: u32, nu: u32, dl: i32, du: i32 }
fn upc(a: u32, b: u32) -> bool { a == UINF || (b != UINF && b <= a) }
fn upc16(a: u16, b: u16) -> bool { a == U16INF || (b != U16INF && b <= a) }
const U16INF: u16 = u16::MAX;
fn contains(c: &T, q: &T) -> bool {
    if q.empty { return true; }
    if c.empty { return false; }
    c.pl <= q.pl && upc(c.pu, q.pu) && c.nl <= q.nl && upc(c.nu, q.nu)
        && (c.dl == NEG || (q.dl != NEG && q.dl >= c.dl)) && (c.du == POS || (q.du != POS && q.du <= c.du))
        && (0..N).all(|i| c.lo[i] <= q.lo[i]) && (0..N).all(|i| upc16(c.up[i], q.up[i]))
}
fn mn(a: Option<i128>, b: Option<i128>) -> Option<i128> { match (a, b) { (Some(a), Some(b)) => Some(a.min(b)), (a, b) => a.or(b) } }
fn mx(a: Option<i128>, b: Option<i128>) -> Option<i128> { match (a, b) { (Some(a), Some(b)) => Some(a.max(b)), (a, b) => a.or(b) } }
fn cu(v: Option<i128>) -> u32 { v.map_or(UINF, |x| x.clamp(0, (UINF - 1) as i128) as u32) }
fn project(phase: u8, owner: u16, lower: &[u64; N], upper: &[Option<u64>; N], rank: Option<u64>, amax: Option<i64>, dmin: Option<i64>, dmax: Option<i64>) -> T {
    let own = |ax: usize| (owner >> ax) & 1 == 1;
    // groups[0]=numerator (inactive), groups[1]=positive (active)
    let mut gl = [0i128; 2]; let mut gu = [0i128; 2]; let mut gun = [0usize; 2]; let mut active = 0i128;
    for ax in 0..N { let g = own(ax) as usize; gl[g] += lower[ax] as i128; match upper[ax] { Some(u) => gu[g] += u as i128, None => gun[g] += 1 } active += own(ax) as i128; }
    let gup = |g: usize| if gun[g] == 0 { Some(gu[g]) } else { None };
    let empty_t = T { phase, owner, empty: true, lo: [0; N], up: [0; N], pl: 0, pu: 0, nl: 0, nu: 0, dl: 0, du: 0 };
    let a_lo = gl[1] + active; let a_up = mn(gup(1).map(|v| v + active), amax.map(|v| v as i128));
    let r_lo = gl[0]; let r_up = mn(gup(0), rank.map(|v| v as i128));
    if a_up.is_some_and(|u| u < a_lo) || r_up.is_some_and(|u| u < r_lo) { return empty_t; }
    let d_lo = mx(dmin.map(|v| v as i128), r_up.map(|u| a_lo - u));
    let d_up = mn(dmax.map(|v| v as i128), a_up.map(|u| u - r_lo));
    if let (Some(l), Some(u)) = (d_lo, d_up) { if l > u { return empty_t; } }
    let pa_lo = d_lo.map_or(a_lo, |d| a_lo.max(r_lo + d));
    let pa_up = mn(a_up, r_up.zip(d_up).map(|(r, d)| r + d));
    let pr_lo = d_up.map_or(r_lo, |d| r_lo.max(a_lo - d));
    let pr_up = mn(r_up, a_up.zip(d_lo).map(|(a, d)| a - d));
    if pa_up.is_some_and(|u| u < pa_lo) || pr_up.is_some_and(|u| u < pr_lo) { return empty_t; }
    let la = (pa_lo - active, pa_up.map(|u| u - active));
    let iv = [(pr_lo, pr_up), la];
    let mut lo = [0u16; N]; let mut up = [U16INF; N];
    for ax in 0..N {
        let g = own(ax) as usize; let (il, iu) = iv[g];
        let other_lower = gl[g] - lower[ax] as i128;
        let rem_unb = gun[g] - upper[ax].is_none() as usize;
        let other_upper = if rem_unb != 0 { None } else { Some(upper[ax].map_or(gu[g], |u| gu[g] - u as i128)) };
        let l = other_upper.map_or(lower[ax] as i128, |o| (lower[ax] as i128).max(il - o));
        let h = mn(upper[ax].map(|u| u as i128), iu.map(|u| u - other_lower));
        lo[ax] = l.clamp(0, 65534) as u16; up[ax] = h.map_or(U16INF, |x| x.clamp(0, 65534) as u16);
    }
    let ci = |v: Option<i128>, inf: i32| v.map_or(inf, |x| x.clamp(i32::MIN as i128 + 1, i32::MAX as i128 - 1) as i32);
    T { phase, owner, empty: false, lo, up, pl: pa_lo.clamp(0, (UINF - 1) as i128) as u32, pu: cu(pa_up), nl: pr_lo.clamp(0, (UINF - 1) as i128) as u32, nu: cu(pr_up), dl: ci(d_lo, NEG), du: ci(d_up, POS) }
}
#[derive(Clone, Copy, PartialEq)]
struct Sig { empty: bool, pos: u32, num: u32, dif: i32 }
fn sig_of(t: &T) -> Sig { Sig { empty: t.empty, pos: t.pu, num: t.nu, dif: t.dl } }
fn sig_may(c: &Sig, q: &Sig) -> bool {
    if q.empty { return true; } if c.empty { return false; }
    upc(c.pos, q.pos) && upc(c.num, q.num) && (c.dif == NEG || (q.dif != NEG && q.dif >= c.dif))
}
struct Block { ids: Vec<u32>, minlo: [u16; N], maxup: [u16; N], minsum: u64, env: bool }
fn block_of(ids: Vec<u32>, doms: &[T]) -> Block {
    let mut minlo = [U16INF; N]; let mut maxup = [0u16; N]; let mut minsum = u64::MAX; let mut env = true;
    for &id in &ids { let t = &doms[id as usize]; if t.empty { env = false; continue; }
        for i in 0..N { minlo[i] = minlo[i].min(t.lo[i]); maxup[i] = if maxup[i] == U16INF || t.up[i] == U16INF { U16INF } else { maxup[i].max(t.up[i]) }; }
        minsum = minsum.min(t.lo.iter().map(|&x| x as u64).sum()); }
    Block { ids, minlo, maxup, minsum, env }
}
fn block_may(b: &Block, q: &T) -> bool {
    q.empty || !b.env || (0..N).all(|i| b.minlo[i] <= q.lo[i] && upc16(b.maxup[i], q.up[i]))
}
struct Group { sig: Sig, blocks: Vec<Block> }
struct Bucket { groups: Vec<Group> }

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1]; let gen = &a[2];
    let samples: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(20000);
    let nodes = fs::read(format!("{dir}/nodes-{gen}.bin")).unwrap();
    let flags = nodes[32..].to_vec(); drop(nodes);
    let n = flags.len();
    let mut doms: Vec<T> = Vec::with_capacity(n);
    let mut segs: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path().to_string_lossy().to_string()).filter(|p| p.contains("/domains-")).collect();
    segs.sort();
    let (mut last_first, mut last_count) = (0usize, 0usize);
    let mut esc = [0usize; 3]; // finite coordinate extremum > 254, finite aggregate > 254, total nonempty
    for path in &segs {
        let b = fs::read(path).unwrap();
        let count = u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize;
        let first = u64::from_le_bytes(b[24..32].try_into().unwrap()) as usize;
        if first + count > n { continue; }
        assert_eq!(first, doms.len());
        let mut r = R { b: &b, p: 32 };
        for _ in 0..count {
            let phase = r.var() as u8; let ol = r.var() as usize; let mut owner = 0u16;
            for k in 0..ol { if r.b[r.p + k] == 1 { owner |= 1 << k; } } r.p += ol;
            assert_eq!(r.var() as usize, N); let mut lo = [0u64; N]; for i in 0..N { lo[i] = r.var() as u64; }
            assert_eq!(r.var() as usize, N); let mut up = [None; N]; for i in 0..N { up[i] = r.opt().map(|v| v as u64); }
            let rank = match r.u8() { 0 => None, 1 => Some(r.var() as u64), x => panic!("rank {x}") };
            let amax = r.opt().map(|v| v as i64); let dmin = r.opt().map(zz); let dmax = r.opt().map(zz);
            let t = project(phase, owner, &lo, &up, rank, amax, dmin, dmax);
            if !t.empty { esc[2] += 1;
                if t.lo.iter().chain(t.up.iter()).any(|&x| x != U16INF && x > 254) { esc[0] += 1; }
                if [t.pl, t.pu, t.nl, t.nu].iter().any(|&x| x != UINF && x > 254) || (t.dl != NEG && (t.dl < -127 || t.dl > 127)) || (t.du != POS && (t.du < -127 || t.du > 127)) { esc[1] += 1; } }
            doms.push(t);
        }
        last_first = first; last_count = count;
    }
    eprintln!("decoded {}", doms.len());
    println!("u8-lane escapes: coord>254 {} aggregate out of u8/i8 {} of nonempty {}", esc[0], esc[1], esc[2]);
    // index
    let idx = fs::read(format!("{dir}/index-{gen}.bin")).unwrap();
    let mut r = R { b: &idx, p: 32 };
    let nb = r.var() as usize;
    let mut stored: Vec<Bucket> = Vec::with_capacity(nb);
    let mut live = vec![false; n];
    let mut key: std::collections::HashMap<(u8, u16), usize> = Default::default();
    let mut sig_mismatch = 0usize; let mut env_violation = 0usize;
    for bi in 0..nb {
        let phase = r.var() as u8; let ol = r.var() as usize; let mut owner = 0u16;
        for k in 0..ol { if r.b[r.p + k] == 1 { owner |= 1 << k; } } r.p += ol;
        key.insert((phase, owner), bi);
        let nids = r.var() as usize; for _ in 0..nids { r.var(); }
        let ng = r.var() as usize; let mut groups = Vec::with_capacity(ng);
        for _ in 0..ng {
            let sig = match r.var() {
                0 => Sig { empty: true, pos: 0, num: 0, dif: 0 },
                1 => { let p = match r.var() { 0 => cu(Some(r.var() as i128)), 1 => UINF, x => panic!("{x}") };
                       let q = match r.var() { 0 => cu(Some(r.var() as i128)), 1 => UINF, x => panic!("{x}") };
                       let d = match r.var() { 0 => NEG, 1 => (zz(r.var()) as i128).clamp(i32::MIN as i128 + 1, i32::MAX as i128 - 1) as i32, x => panic!("{x}") };
                       Sig { empty: false, pos: p, num: q, dif: d } }
                x => panic!("sig {x}") };
            let nbl = r.var() as usize; let mut blocks = Vec::with_capacity(nbl);
            for _ in 0..nbl {
                let mut ids = [0u32; 32]; for i in 0..32 { ids[i] = r.var() as u32; }
                let len = r.var() as usize; let env = r.var() as usize;
                let mut minlo = [0u16; N]; let mut maxup = [U16INF; N];
                for ax in 0..env { let a1 = r.var(); r.var(); r.opt(); let a4 = r.opt(); if ax < N { minlo[ax] = a1.min(65534) as u16; maxup[ax] = a4.map_or(U16INF, |v| v.min(65534) as u16); } }
                for &id in &ids[..len] { live[id as usize] = true; let t = &doms[id as usize];
                    if sig_of(t) != sig { sig_mismatch += 1; }
                    if env == N && !t.empty && !(0..N).all(|i| minlo[i] <= t.lo[i] && upc16(maxup[i], t.up[i])) { env_violation += 1; } }
                blocks.push(Block { ids: ids[..len].to_vec(), minlo, maxup, minsum: 0, env: env == N });
            }
            r.var();
            groups.push(Group { sig, blocks });
        }
        r.var(); r.opt();
        stored.push(Bucket { groups });
    }
    assert_eq!(r.p, idx.len()); drop(idx);
    println!("check: live ids whose tight signature != stored group signature: {sig_mismatch}; stored envelope violations: {env_violation}");
    // ---- layout experiments: flat 64-blocks and 2-level hierarchy (64 blocks per superblock) ----
    struct HGroup { sig: Sig, blocks: Vec<Block>, supers: Vec<(Block, usize, usize)> } // super envelope, block range
    fn morton(t: &T, with_upper: bool) -> [u64; 4] {
        // 8 bits per dim; dims = lo (15) [+ inverted up (15)]; interleave MSB first
        let mut dims: Vec<u8> = t.lo.iter().map(|&x| x.min(255) as u8).collect();
        if with_upper { for &u in &t.up { dims.push(if u == U16INF { 0 } else { 255 - u.min(255) as u8 }); } }
        let mut out = [0u64; 4]; let mut pos = 0usize;
        for bit in (0..8).rev() { for &d in &dims { let b = (d >> bit) & 1; if b == 1 { out[pos / 64] |= 1u64 << (63 - pos % 64); } pos += 1; } }
        out
    }
    let build = |kind: u8, frac_permille: u64| -> Vec<Vec<HGroup>> {
        stored.iter().map(|b| b.groups.iter().map(|g| {
            let mut ids: Vec<u32> = g.blocks.iter().flat_map(|bl| bl.ids.iter().copied())
                .filter(|&id| (id as u64).wrapping_mul(0x9E3779B97F4A7C15) >> 54 < frac_permille).collect();
            match kind {
                0 => ids.sort_unstable(), // ID order (today)
                1 => ids.sort_by_cached_key(|&id| { let t = &doms[id as usize]; let mut k = [0u16; 16];
                        k[0] = (0..N).map(|i| ((t.up[i] != U16INF) as u16) << i).sum(); for i in 0..N { k[i + 1] = t.lo[i]; } k }),
                2 => ids.sort_by_cached_key(|&id| morton(&doms[id as usize], false)),
                3 => ids.sort_by_cached_key(|&id| morton(&doms[id as usize], true)),
                _ => unreachable!(),
            }
            let blocks: Vec<Block> = ids.chunks(64).map(|c| block_of(c.to_vec(), &doms)).collect();
            let mut supers = Vec::new(); let mut s = 0;
            while s < blocks.len() { let e = (s + 64).min(blocks.len());
                let all: Vec<u32> = blocks[s..e].iter().flat_map(|bl| bl.ids.iter().copied()).collect();
                supers.push((block_of(all, &doms), s, e)); s = e; }
            HGroup { sig: g.sig, blocks, supers } }).collect()).collect()
    };
    let hscan = |q: usize, layout: &Vec<Vec<HGroup>>, first_found: bool, excl: bool| -> (bool, usize, usize, usize) {
        let t = &doms[q];
        let Some(&bi) = key.get(&(t.phase, t.owner)) else { return (false, 0, 0, 0) };
        let qs = sig_of(t); let qsum: u64 = t.lo.iter().map(|&x| x as u64).sum();
        let (mut found, mut se, mut be, mut te) = (false, 0usize, 0usize, 0usize);
        'g: for g in &layout[bi] {
            if !sig_may(&g.sig, &qs) { continue; }
            for (sb, s, e) in &g.supers {
                se += 1;
                if sb.env && sb.minsum > qsum && !t.empty { continue; }
                if !block_may(sb, t) { continue; }
                for bl in &g.blocks[*s..*e] {
                    be += 1;
                    if bl.env && bl.minsum > qsum && !t.empty { continue; }
                    if !block_may(bl, t) { continue; }
                    for &id in &bl.ids { if excl && id as usize == q { continue; } te += 1;
                        if contains(&doms[id as usize], t) { found = true; if first_found { break 'g; } } }
                }
            }
        }
        (found, se, be, te)
    };
    let mut rng: u64 = 0x9e3779b97f4a7c15;
    let mut pick = |m: usize| -> usize { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; (rng % m as u64) as usize };
    let miss_pool: Vec<usize> = (last_first..last_first + last_count).filter(|&id| live[id]).collect();
    let hit_pool: Vec<usize> = (0..n).filter(|&id| flags[id] & 2 == 0 && !live[id]).collect();
    let misses: Vec<usize> = (0..samples.min(miss_pool.len())).map(|_| miss_pool[pick(miss_pool.len())]).collect();
    let hits: Vec<usize> = (0..samples.min(hit_pool.len())).map(|_| hit_pool[pick(hit_pool.len())]).collect();
    let names = ["id-order", "finite-pattern+lex-lower", "morton(lower)", "morton(lower,-upper)"];
    for kind in [0u8, 1, 2, 3] { for frac in [1024u64, 256] {
        let lay = build(kind, frac);
        for (label, qs, ff, excl) in [("MISS", &misses, false, true), ("HIT-ff", &hits, true, false)] {
            let t0 = std::time::Instant::now();
            let (mut f, mut se, mut be, mut te) = (0usize, 0usize, 0usize, 0usize); let mut tv = Vec::with_capacity(qs.len());
            for &q in qs.iter() { let (a1, a2, a3, a4) = hscan(q, &lay, ff, excl); f += a1 as usize; se += a2; be += a3; te += a4; tv.push(a4); }
            tv.sort_unstable(); let k = tv.len(); let nq = qs.len() as f64;
            println!("{:26} live {:5.1}% {:6}: found {:.3} super-env {:7.0} block-env {:7.0} tested {:7.0} [p50 {} p90 {} p99 {}] total-work(env+tested) {:7.0} wall {:.1} us/q",
                names[kind as usize], 100.0 * frac as f64 / 1024.0, label, f as f64 / nq, se as f64 / nq, be as f64 / nq, te as f64 / nq, tv[k / 2], tv[k * 9 / 10], tv[k * 99 / 100], (se + be + te) as f64 / nq, 1e6 * t0.elapsed().as_secs_f64() / nq);
        }
    } }
}
