use std::fs;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let b = fs::read(&a[1]).unwrap();
    let body = &b[32..];
    let m = body.len() / 8;
    let mut deg: std::collections::HashMap<u32, u32> = Default::default();
    let mut last = u32::MAX; let mut runs = 0usize;
    for i in 0..m { let s = u32::from_le_bytes(body[8*i..8*i+4].try_into().unwrap()); *deg.entry(s).or_default() += 1; if s != last { runs += 1; last = s; } }
    let mut bins = [(0usize, 0usize); 24];
    for (_, &d) in &deg { let k = (32 - d.leading_zeros()) as usize; bins[k].0 += 1; bins[k].1 += d as usize; }
    println!("edges {m} sources {} source-runs {runs}", deg.len());
    let mut cum = 0usize;
    for k in (0..24).rev() { if bins[k].0 == 0 { continue; } cum += bins[k].1;
        println!("outdeg [{}, {}): sources {} edges {} ({:.1}%) cum-from-top {:.1}%", if k == 0 {0} else {1u64 << (k-1)}, 1u64 << k, bins[k].0, bins[k].1, 100.0*bins[k].1 as f64/m as f64, 100.0*cum as f64/m as f64); }
}
