// Read-only replay of admission lookups against a CP5 live index (perf skeptic).
// Decodes domains (raw boxes) + index (buckets/groups/blocks/envelopes).
// Miss proxies: live IDs admitted in the newest segment (full scan, self excluded).
// Hit proxies: delegated/retired IDs (contained in some live candidate).
// Containment here = raw syntactic box test (Domain::contains reference form),
// a sufficient condition only; tight-summary semantics may find more hits.
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
const NONE16: u16 = u16::MAX;
#[derive(Clone, Copy)]
struct Dom { phase: u8, owner: u16, lo: [u16; N], up: [u16; N], rank: u16, a: u32, dmin: i32, dmax: i32 }
const INF32: u32 = u32::MAX; const NEG: i32 = i32::MIN; const POS: i32 = i32::MAX;
fn contains(c: &Dom, q: &Dom) -> bool {
    // rank: None(=u16::MAX) contains all; finite r contains finite s<=r
    let rk = c.rank == NONE16 || (q.rank != NONE16 && q.rank <= c.rank);
    let a = c.a == INF32 || (q.a != INF32 && q.a <= c.a);
    let dl = c.dmin == NEG || (q.dmin != NEG && q.dmin >= c.dmin);
    let du = c.dmax == POS || (q.dmax != POS && q.dmax <= c.dmax);
    rk && a && dl && du
        && (0..N).all(|i| c.lo[i] <= q.lo[i])
        && (0..N).all(|i| c.up[i] == NONE16 || (q.up[i] != NONE16 && q.up[i] <= c.up[i]))
}
#[derive(Clone, Copy)]
struct Sig { empty: bool, pos: Option<u128>, num: Option<u128>, dif: Option<i64> }
fn up_c(a: Option<u128>, b: Option<u128>) -> bool { a.is_none() || (b.is_some() && b.unwrap() <= a.unwrap()) }
fn sig_may(c: &Sig, q: &Sig) -> bool {
    if q.empty { return true; } if c.empty { return false; }
    up_c(c.pos, q.pos) && up_c(c.num, q.num) && (c.dif.is_none() || (q.dif.is_some() && q.dif.unwrap() >= c.dif.unwrap()))
}
struct Block { ids: Vec<u32>, minlo: [u16; N], maxup: [u16; N], has_env: bool }
struct Group { sig: Sig, blocks: Vec<Block>, live: usize }
struct Bucket { phase: u8, owner: u16, groups: Vec<Group>, live: usize }

fn block_may(b: &Block, q: &Dom) -> bool {
    !b.has_env || (0..N).all(|i| b.minlo[i] <= q.lo[i] && (b.maxup[i] == NONE16 || (q.up[i] != NONE16 && q.up[i] <= b.maxup[i])))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1]; let gen = &a[2];
    let samples: usize = a.get(3).map(|s| s.parse().unwrap()).unwrap_or(20000);
    let nodes = fs::read(format!("{dir}/nodes-{gen}.bin")).unwrap();
    let flags = nodes[32..].to_vec(); drop(nodes);
    let n = flags.len();
    // domains
    let mut doms: Vec<Dom> = Vec::with_capacity(n);
    let mut segs: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().path().to_string_lossy().to_string())
        .filter(|p| p.contains("/domains-")).collect();
    segs.sort();
    let mut last_first = 0usize; let mut last_count = 0usize;
    for path in &segs {
        let b = fs::read(path).unwrap();
        let count = u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize;
        let first = u64::from_le_bytes(b[24..32].try_into().unwrap()) as usize;
        if first + count > n { eprintln!("skip segment beyond nodes: {path}"); continue; }
        assert_eq!(first, doms.len(), "segment order {path}");
        let mut r = R { b: &b, p: 32 };
        for _ in 0..count {
            let phase = r.var() as u8;
            let ol = r.var() as usize; let mut owner = 0u16;
            for k in 0..ol { if r.b[r.p + k] == 1 { owner |= 1 << k; } } r.p += ol;
            let ll = r.var() as usize; assert_eq!(ll, N);
            let mut lo = [0u16; N]; for i in 0..N { lo[i] = r.var().min(65534) as u16; }
            let ul = r.var() as usize; assert_eq!(ul, N);
            let mut up = [NONE16; N]; for i in 0..N { if let Some(v) = r.opt() { up[i] = v.min(65534) as u16; } }
            let rank = match r.u8() { 0 => NONE16, 1 => r.var().min(65534) as u16, x => panic!("rank {x}") };
            let aa = r.opt().map(|v| v.min(4e9 as u128) as u32).unwrap_or(INF32);
            let dmin = r.opt().map(|v| zz(v).clamp(-2_000_000_000, 2_000_000_000) as i32).unwrap_or(NEG);
            let dmax = r.opt().map(|v| zz(v).clamp(-2_000_000_000, 2_000_000_000) as i32).unwrap_or(POS);
            doms.push(Dom { phase, owner, lo, up, rank, a: aa, dmin, dmax });
        }
        assert_eq!(r.p, b.len());
        last_first = first; last_count = count;
        eprintln!("{path}: first {first} count {count}");
    }
    eprintln!("decoded {} domains of {n}", doms.len());
    // index
    let idx = fs::read(format!("{dir}/index-{gen}.bin")).unwrap();
    let mut r = R { b: &idx, p: 32 };
    let nb = r.var() as usize;
    let mut buckets: Vec<Bucket> = Vec::with_capacity(nb);
    let mut loc: Vec<(u32, u32)> = vec![(u32::MAX, u32::MAX); n]; // live id -> (bucket, group)
    for bi in 0..nb {
        let phase = r.var() as u8; let ol = r.var() as usize; let mut owner = 0u16;
        for k in 0..ol { if r.b[r.p + k] == 1 { owner |= 1 << k; } } r.p += ol;
        let nids = r.var() as usize; for _ in 0..nids { r.var(); }
        let ng = r.var() as usize; let mut groups = Vec::with_capacity(ng);
        for gi in 0..ng {
            let sig = match r.var() {
                0 => Sig { empty: true, pos: None, num: None, dif: None },
                1 => {
                    let pos = match r.var() { 0 => Some(r.var()), 1 => None, x => panic!("up {x}") };
                    let num = match r.var() { 0 => Some(r.var()), 1 => None, x => panic!("up {x}") };
                    let dif = match r.var() { 0 => None, 1 => Some(zz(r.var())), x => panic!("lo {x}") };
                    Sig { empty: false, pos, num, dif }
                }
                x => panic!("sig {x}"),
            };
            let nbl = r.var() as usize; let mut blocks = Vec::with_capacity(nbl);
            for _ in 0..nbl {
                let mut ids = [0u32; 32]; for i in 0..32 { ids[i] = r.var() as u32; }
                let len = r.var() as usize; let env = r.var() as usize;
                let mut minlo = [0u16; N]; let mut maxup = [NONE16; N];
                for ax in 0..env { let mnl = r.var(); r.var(); r.opt(); let mxu = r.opt();
                    if ax < N { minlo[ax] = mnl.min(65534) as u16; maxup[ax] = mxu.map(|v| v.min(65534) as u16).unwrap_or(NONE16); } }
                for &id in &ids[..len] { loc[id as usize] = (bi as u32, gi as u32); }
                blocks.push(Block { ids: ids[..len].to_vec(), minlo, maxup, has_env: env == N });
            }
            let live = r.var() as usize;
            groups.push(Group { sig, blocks, live });
        }
        let live = r.var() as usize; r.opt();
        buckets.push(Bucket { phase, owner, groups, live });
    }
    assert_eq!(r.p, idx.len()); drop(idx);
    let total_live: usize = buckets.iter().map(|b| b.live).sum();
    println!("gen {gen} nodes {n} buckets {nb} live {total_live}");
    // per-bucket: new IDs in last segment (miss events of the last window), and traffic proxies
    let mut new_in_bucket = vec![0usize; nb];
    let mut key_to_bucket = std::collections::HashMap::<(u8, u16), usize>::new();
    for (i, b) in buckets.iter().enumerate() { key_to_bucket.insert((b.phase, b.owner), i); }
    for id in last_first..last_first + last_count { if let Some(&bi) = key_to_bucket.get(&(doms[id].phase, doms[id].owner)) { new_in_bucket[bi] += 1; } }
    let mut order: Vec<usize> = (0..nb).collect(); order.sort_by_key(|&i| std::cmp::Reverse(buckets[i].live));
    println!("top buckets by live: phase owner live groups blocks new_ids_last_segment");
    for &i in order.iter().take(12) { let b = &buckets[i];
        println!("  {} {:015b} live {} groups {} blocks {} new_last_seg {}", b.phase, b.owner.reverse_bits() >> 1, b.live, b.groups.len(), b.groups.iter().map(|g| g.blocks.len()).sum::<usize>(), new_in_bucket[i]); }
    let mut ord2: Vec<usize> = (0..nb).collect(); ord2.sort_by_key(|&i| std::cmp::Reverse(new_in_bucket[i]));
    let tot_new: usize = new_in_bucket.iter().sum();
    println!("top buckets by new IDs in last segment (miss share), total {tot_new}:");
    for &i in ord2.iter().take(8) { let b = &buckets[i];
        println!("  {} {:015b} new {} ({:.1}%) live {} groups {}", b.phase, b.owner.reverse_bits() >> 1, new_in_bucket[i], 100.0 * new_in_bucket[i] as f64 / tot_new as f64, b.live, b.groups.len()); }
    // weighted: expected live size of the bucket a miss lands in
    let wlive: f64 = (0..nb).map(|i| new_in_bucket[i] as f64 * buckets[i].live as f64).sum::<f64>() / tot_new as f64;
    println!("miss-weighted mean bucket live size {:.0}", wlive);

    // scans
    let mut rng: u64 = 0x9e3779b97f4a7c15;
    let mut next = |m: usize| -> usize { rng ^= rng << 13; rng ^= rng >> 7; rng ^= rng << 17; (rng % m as u64) as usize };
    // miss proxies: live ids in last segment
    let miss_pool: Vec<usize> = (last_first..last_first + last_count).filter(|&id| loc[id].0 != u32::MAX).collect();
    // hit proxies: not inspected, not live (delegated sealed or retired pending), across all ids
    let hit_pool: Vec<usize> = (0..n).filter(|&id| flags[id] & 2 == 0 && loc[id].0 == u32::MAX).collect();
    println!("miss_pool {} hit_pool {}", miss_pool.len(), hit_pool.len());
    let scan = |q: usize, first_found: bool, exclude_self: bool| -> (usize, usize, usize, usize, bool) {
        // returns (eligible_group_candidates, blocks_visited, candidates_tested, groups_eligible, found)
        let d = &doms[q];
        let Some(&bi) = key_to_bucket.get(&(d.phase, d.owner)) else { return (0, 0, 0, 0, false) };
        let b = &buckets[bi];
        // query signature: use group signature if live, else derive loosely from box (approx: raw A/rank/dmin)
        let qsig = if loc[q].0 != u32::MAX { b.groups[loc[q].1 as usize].sig } else {
            Sig { empty: false, pos: if d.a == INF32 { None } else { Some(d.a as u128) }, num: if d.rank == NONE16 { None } else { Some(d.rank as u128) }, dif: if d.dmin == NEG { None } else { Some(d.dmin as i64) } } };
        let (mut elig_c, mut bv, mut tested, mut ge) = (0usize, 0usize, 0usize, 0usize);
        let mut best: Option<u32> = None;
        'g: for g in &b.groups {
            if !sig_may(&g.sig, &qsig) { continue; }
            ge += 1; elig_c += g.live;
            for bl in &g.blocks {
                if !first_found { if let Some(bst) = best { if bl.ids.first().map_or(true, |&f| f >= bst) { break; } } }
                if !block_may(bl, d) { continue; }
                bv += 1;
                for &id in &bl.ids {
                    if exclude_self && id as usize == q { continue; }
                    if !first_found { if let Some(bst) = best { if id >= bst { break; } } }
                    tested += 1;
                    if contains(&doms[id as usize], d) { if best.map_or(true, |x| id < x) { best = Some(id); } if first_found { break 'g; } break; }
                }
            }
        }
        (elig_c, bv, tested, ge, best.is_some())
    };
    let ms = samples.min(miss_pool.len());
    let (mut e, mut bv, mut t, mut ge, mut f) = (0f64, 0f64, 0f64, 0f64, 0usize);
    let mut tv: Vec<usize> = Vec::with_capacity(ms);
    for _ in 0..ms { let q = miss_pool[next(miss_pool.len())]; let (a1, a2, a3, a4, a5) = scan(q, false, true); e += a1 as f64; bv += a2 as f64; t += a3 as f64; ge += a4 as f64; f += a5 as usize; tv.push(a3); }
    tv.sort_unstable();
    println!("MISS proxies n={ms}: mean eligible-group candidates {:.0}, blocks visited {:.0}, candidates tested (block-pruned, raw coords) {:.0} [p50 {} p90 {} p99 {}], eligible groups {:.1}, unexpectedly found container {}",
        e / ms as f64, bv / ms as f64, t / ms as f64, tv[ms / 2], tv[ms * 9 / 10], tv[ms * 99 / 100], ge / ms as f64, f);
    for ff in [false, true] {
        let hs = samples.min(hit_pool.len());
        let (mut e, mut bv, mut t, mut f) = (0f64, 0f64, 0f64, 0usize);
        let mut tv: Vec<usize> = Vec::with_capacity(hs);
        let mut rng2: u64 = 0x1234_5678_9abc_def1;
        for _ in 0..hs { rng2 ^= rng2 << 13; rng2 ^= rng2 >> 7; rng2 ^= rng2 << 17; let q = hit_pool[(rng2 % hit_pool.len() as u64) as usize];
            let (a1, a2, a3, _a4, a5) = scan(q, ff, false); e += a1 as f64; bv += a2 as f64; t += a3 as f64; f += a5 as usize; tv.push(a3); }
        tv.sort_unstable();
        println!("HIT proxies ({}) n={hs}: found {:.3}; mean eligible-group candidates {:.0}, blocks visited {:.0}, candidates tested {:.0} [p50 {} p90 {} p99 {}]",
            if ff { "first-found" } else { "min-ID" }, f as f64 / hs as f64, e / hs as f64, bv / hs as f64, t / hs as f64, tv[hs / 2], tv[hs * 9 / 10], tv[hs * 99 / 100]);
    }
}
