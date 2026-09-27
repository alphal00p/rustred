// Scratch microbenchmark: random gather vs streaming dominance scan, 4K vs THP.
use std::time::Instant;

extern "C" {
    fn madvise(addr: *mut u8, len: usize, advice: i32) -> i32;
    fn posix_memalign(memptr: *mut *mut u8, alignment: usize, size: usize) -> i32;
}
const MADV_HUGEPAGE: i32 = 14;
const MADV_NOHUGEPAGE: i32 = 15;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

fn alloc(bytes: usize, huge: bool) -> *mut u8 {
    let mut p: *mut u8 = std::ptr::null_mut();
    unsafe {
        assert_eq!(posix_memalign(&mut p, 2 << 20, bytes), 0);
        madvise(p, bytes, if huge { MADV_HUGEPAGE } else { MADV_NOHUGEPAGE });
        std::ptr::write_bytes(p, 0, bytes); // first touch
    }
    p
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let huge = args.get(1).map(|s| s == "huge").unwrap_or(false);
    let n: usize = 45_000_000; // IDs, like the v2 queue
    // A: dependent pointer chase over n u64 (360 MB)
    let chase = alloc(n * 8, huge) as *mut u64;
    let chase = unsafe { std::slice::from_raw_parts_mut(chase, n) };
    let mut perm: Vec<u32> = (0..n as u32).collect();
    let mut rng = Rng(0x9e3779b97f4a7c15);
    for i in (1..n).rev() {
        let j = (rng.next() % (i as u64 + 1)) as usize;
        perm.swap(i, j);
    }
    for i in 0..n {
        chase[perm[i] as usize] = perm[(i + 1) % n] as u64;
    }
    let steps = 5_000_000;
    let mut p = perm[0] as u64;
    let t = Instant::now();
    for _ in 0..steps {
        p = chase[p as usize];
    }
    let dep_ns = t.elapsed().as_nanos() as f64 / steps as f64;
    // B: independent random gathers of 8 B words by ID (like bits[id])
    let m = 20_000_000;
    let idx: Vec<u32> = (0..m).map(|_| (rng.next() % n as u64) as u32).collect();
    let t = Instant::now();
    let mut s = 0u64;
    for &i in &idx {
        s = s.wrapping_add(chase[i as usize] & 0xff);
    }
    let gather_ns = t.elapsed().as_nanos() as f64 / m as f64;
    // C: independent gathers of 64-B summaries by ID (like summaries.get(id)) + 36-lane test
    let rec = 64usize;
    let recs = 45_000_000usize;
    let base = alloc(recs * rec, huge);
    let summ = unsafe { std::slice::from_raw_parts_mut(base, recs * rec) };
    for (k, b) in summ.iter_mut().enumerate() {
        *b = (k as u64).wrapping_mul(0x9E37_79B9).rotate_left(7) as u8 & 0x3f;
    }
    let q: [u8; 64] = [40u8; 64];
    let t = Instant::now();
    let mut hits = 0u64;
    for &i in &idx {
        let r = &summ[i as usize * rec..i as usize * rec + rec];
        if r.iter().zip(q.iter()).take(36).all(|(a, b)| a <= b) {
            hits += 1;
        }
    }
    let gather_test_ns = t.elapsed().as_nanos() as f64 / m as f64;
    // D: streaming scan over contiguous 64-B records, same test, no early exit
    let scan = 20_000_000usize;
    let t = Instant::now();
    let mut hits2 = 0u64;
    for c in summ[..scan * rec].chunks_exact(rec) {
        let mut ok = true;
        for j in 0..36 {
            ok &= c[j] <= q[j];
        }
        hits2 += ok as u64;
    }
    let stream_ns = t.elapsed().as_nanos() as f64 / scan as f64;
    // E: SoA (lane-major) blocks of 64 candidates: per lane one 64-byte row
    let blocks = scan / 64;
    let t = Instant::now();
    let mut hits3 = 0u64;
    for b in 0..blocks {
        let blk = &summ[b * 64 * 36..b * 64 * 36 + 64 * 36];
        let mut mask = u64::MAX;
        for j in 0..36 {
            let row = &blk[j * 64..j * 64 + 64];
            let mut m = 0u64;
            for k in 0..64 {
                m |= ((row[k] <= q[j]) as u64) << k;
            }
            mask &= m;
            if mask == 0 {
                break;
            }
        }
        hits3 += mask.count_ones() as u64;
    }
    let soa_ns = t.elapsed().as_nanos() as f64 / (blocks * 64) as f64;
    println!(
        "{{\"huge\":{},\"dependent_chase_ns\":{:.1},\"independent_gather8_ns\":{:.2},\"gather64_plus_test_ns\":{:.2},\"stream_aos_test_ns\":{:.3},\"soa_block_test_ns\":{:.3},\"sink\":[{},{},{},{},{}]}}",
        huge, dep_ns, gather_ns, gather_test_ns, stream_ns, soa_ns, p, s, hits, hits2, hits3
    );
}
