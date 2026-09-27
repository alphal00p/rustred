use std::io::Read;
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let mut f = std::fs::File::open(&p).unwrap();
    let mut hdr = [0u8; 32];
    f.read_exact(&mut hdr).unwrap();
    let count = u64::from_le_bytes(hdr[16..24].try_into().unwrap()) as usize;
    let first = u64::from_le_bytes(hdr[24..32].try_into().unwrap());
    eprintln!("count {count} first {first}");
    let mut indeg: Vec<u32> = Vec::new();
    let mut outdeg: Vec<u32> = Vec::new();
    let mut buf = vec![0u8; 8 << 20];
    let mut n = 0usize; let mut lt67 = 0usize; let mut lt183=0usize; let mut fwd=0usize;
    loop {
        let r = f.read(&mut buf).unwrap();
        if r == 0 { break; }
        assert!(r % 8 == 0 || true);
        let mut i = 0;
        while i + 8 <= r {
            let s = u32::from_le_bytes(buf[i..i+4].try_into().unwrap()) as usize;
            let t = u32::from_le_bytes(buf[i+4..i+8].try_into().unwrap()) as usize;
            if t >= indeg.len() { indeg.resize(t + 1 + (1<<20), 0); }
            if s >= outdeg.len() { outdeg.resize(s + 1 + (1<<20), 0); }
            indeg[t] += 1; outdeg[s] += 1;
            if t < 67 { lt67 += 1; } if t < 183 { lt183 += 1; } if t > s { fwd += 1; }
            n += 1; i += 8;
        }
        if r % 8 != 0 { panic!("partial read"); }
    }
    println!("edges {n} to<67 {:.4} to<183 {:.4} t>s {:.4}", lt67 as f64 / n as f64, lt183 as f64/n as f64, fwd as f64/n as f64);
    let mut v: Vec<u32> = indeg.iter().copied().filter(|&x| x > 0).collect();
    v.sort_unstable_by(|a, b| b.cmp(a));
    println!("distinct targets {}", v.len());
    let mut cum = 0u64; let mut k = 0usize;
    for mark in [1usize, 10, 67, 100, 1000, 10000, 100000, 1000000, 10000000] {
        while k < mark && k < v.len() { cum += v[k] as u64; k += 1; }
        println!("top {mark}: {:.4}", cum as f64 / n as f64);
    }
    let mut top: Vec<(u32, usize)> = indeg.iter().copied().enumerate().map(|(i, c)| (c, i)).filter(|x| x.0 > 0).collect();
    top.sort_unstable_by(|a, b| b.cmp(a));
    println!("top targets: {:?}", &top[..30.min(top.len())]);
    let srcs = outdeg.iter().filter(|&&x| x > 0).count();
    println!("distinct sources {srcs}");
    // in-degree histogram
    let mut h = [0usize; 12];
    for &c in &v { let b = (32 - c.leading_zeros()) as usize; h[b.min(11)] += 1; }
    println!("indeg log2 hist {:?}", h);
}
