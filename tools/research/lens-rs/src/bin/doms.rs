// Per-ID owner support / phase / rank from CP5 domain segments; joins with
// node flags and the live-index bitmap (recomputed) for mistake census.
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
fn live_bitmap(idx: &[u8], n: usize) -> Vec<bool> {
    let mut live = vec![false; n];
    let mut r = R { b: idx, p: 32 };
    let buckets = r.var() as usize;
    for _ in 0..buckets {
        r.var(); let olen = r.var() as usize; r.p += olen;
        let nids = r.var() as usize; for _ in 0..nids { r.var(); }
        let ng = r.var() as usize;
        for _ in 0..ng {
            match r.var() { 0 => {}, 1 => { for _ in 0..2 { if r.var() == 0 { r.var(); } } if r.var() == 1 { r.var(); } }, x => panic!("sig {x}") }
            let nb = r.var() as usize;
            for _ in 0..nb {
                let mut ids = [0usize; 32];
                for i in 0..32 { ids[i] = r.var() as usize; }
                let len = r.var() as usize;
                let env = r.var() as usize;
                for _ in 0..env { r.var(); r.var(); r.opt(); r.opt(); }
                for &id in &ids[..len] { if id < n { live[id] = true; } }
            }
            r.var();
        }
        r.var(); r.opt();
    }
    live
}
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1];
    let gen = &a[2];
    let nodes = fs::read(format!("{dir}/nodes-{gen}.bin")).unwrap();
    let flags = &nodes[32..];
    let n = flags.len();
    let idx = fs::read(format!("{dir}/index-{gen}.bin")).unwrap();
    let live = live_bitmap(&idx, n);
    drop(idx);
    // support, phase, rank per id
    let mut sup = vec![255u8; n];
    let mut phase = vec![255u8; n];
    let mut rank = vec![255u8; n];
    let mut segs: Vec<(u64, String)> = Vec::new();
    for e in fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        if name.starts_with("domains-") { segs.push((0, p.to_string_lossy().to_string())); }
    }
    segs.sort_by(|a, b| a.1.cmp(&b.1));
    let mut seen = 0usize;
    for (_, path) in &segs {
        let b = fs::read(path).unwrap();
        let count = u64::from_le_bytes(b[16..24].try_into().unwrap()) as usize;
        let first = u64::from_le_bytes(b[24..32].try_into().unwrap()) as usize;
        let mut r = R { b: &b, p: 32 };
        for k in 0..count {
            let id = first + k;
            let ph = r.var() as u8;
            let ol = r.var() as usize;
            let s = r.b[r.p..r.p+ol].iter().filter(|&&x| x == 1).count() as u8;
            r.p += ol;
            let ll = r.var() as usize; for _ in 0..ll { r.var(); }
            let ul = r.var() as usize; for _ in 0..ul { r.opt(); }
            let rk = match r.u8() { 0 => 255u8, 1 => r.var().min(254) as u8, x => panic!("rank opt {x}") };
            r.opt(); r.opt(); r.opt();
            if id < n { sup[id] = s; phase[id] = ph; rank[id] = rk; seen += 1; }
        }
        assert_eq!(r.p, b.len(), "segment {path} trailing");
        eprintln!("{path}: first {first} count {count}");
    }
    println!("decoded_domains {seen} of nodes {n}");
    // table by (phase, support): inspected, inspected_not_live, delegated, pending_live, pending_not_live
    let mut t = std::collections::BTreeMap::<(u8, u8), [usize; 5]>::new();
    let mut tr = std::collections::BTreeMap::<u8, [usize; 5]>::new();
    for id in 0..n {
        let f = flags[id];
        let (i, s, l) = (f & 2 != 0, f & 1 != 0, live[id]);
        let k = match (i, s, l) { (true, _, false) => 1, (true, _, true) => 0, (false, true, _) => 2, (false, false, true) => 3, (false, false, false) => 4 };
        let e = t.entry((phase[id], sup[id])).or_default(); e[k] += 1;
        let e = tr.entry(rank[id]).or_default(); e[k] += 1;
    }
    println!("phase support | insp_live insp_covered delegated pend_live pend_covered | covered_share_of_inspected");
    for ((ph, s), c) in &t {
        let insp = c[0] + c[1];
        println!("{ph} {s} | {} {} {} {} {} | {:.3}", c[0], c[1], c[2], c[3], c[4], if insp > 0 { c[1] as f64 / insp as f64 } else { 0.0 });
    }
    println!("rank | insp_live insp_covered delegated pend_live pend_covered | covered_share");
    for (rk, c) in &tr {
        let insp = c[0] + c[1];
        println!("{rk} | {} {} {} {} {} | {:.3}", c[0], c[1], c[2], c[3], c[4], if insp > 0 { c[1] as f64 / insp as f64 } else { 0.0 });
    }
    // covered share by ID decile (time proxy)
    println!("id_decile | inspected covered_share");
    for d in 0..10 {
        let (lo, hi) = (n * d / 10, n * (d + 1) / 10);
        let (mut ins, mut cov) = (0usize, 0usize);
        for id in lo..hi { let f = flags[id]; if f & 2 != 0 { ins += 1; if !live[id] { cov += 1; } } }
        println!("{d} | {ins} {:.3}", if ins > 0 { cov as f64 / ins as f64 } else { 0.0 });
    }
}
