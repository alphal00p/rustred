// Read-only: creator of each domain from CP5 edge segments; delegated records vs representative creators.
// usage: deleg <total_domains> <delegated_lines.jsonl> <edges files...>
use std::io::{BufRead, BufReader, Read};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let total: usize = a[1].parse().unwrap();
    let deleg = &a[2];
    let mut creator = vec![u32::MAX; total];
    let mut outdeg = vec![0u32; total];
    let mut edges = 0u64;
    for f in &a[3..] {
        let mut fh = std::fs::File::open(f).unwrap();
        let mut head = [0u8; 32];
        fh.read_exact(&mut head).unwrap();
        let mut r = BufReader::with_capacity(1 << 24, fh);
        let mut buf = vec![0u8; 8 << 20];
        loop {
            let n = r.read(&mut buf).unwrap();
            if n == 0 { break; }
            let mut m = n;
            while m % 8 != 0 { let k = r.read(&mut buf[m..m + (8 - m % 8)]).unwrap(); assert!(k > 0); m += k; }
            for p in buf[..m].chunks_exact(8) {
                let s = u32::from_le_bytes(p[0..4].try_into().unwrap());
                let t = u32::from_le_bytes(p[4..8].try_into().unwrap()) as usize;
                edges += 1;
                outdeg[s as usize] += 1;
                if creator[t] == u32::MAX { creator[t] = s; }
            }
        }
        eprintln!("read {f} edges so far {edges}");
    }
    // domains created per source
    let mut created = vec![0u32; total];
    let mut no_creator = 0usize;
    for t in 0..total { if creator[t] != u32::MAX { created[creator[t] as usize] += 1; } else { no_creator += 1; } }
    let mut hist_created = std::collections::BTreeMap::new();
    for s in 0..total { if outdeg[s] > 0 { *hist_created.entry(created[s].min(1000).next_power_of_two()).or_insert(0usize) += 1; } }
    println!("edges={edges} no_creator={no_creator} created_hist(pow2 bucket -> sources)={:?}", hist_created);
    // delegated records
    let fh = std::fs::File::open(deleg).unwrap();
    let r = BufReader::with_capacity(1 << 24, fh);
    let (mut n, mut same, mut rep_later, mut rep_earlier, mut rep_by_child, mut unknown) = (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
    let mut gap = std::collections::BTreeMap::new();
    for line in r.lines() {
        let line = line.unwrap();
        let v: serde_json::Value = match serde_json::from_str(&line) { Ok(v) => v, Err(_) => continue };
        let id = v["id"].as_u64().unwrap() as usize;
        let rep = match v["representative_id"].as_u64() { Some(x) => x as usize, None => { unknown += 1; continue } };
        n += 1;
        if rep > id { rep_later += 1 } else { rep_earlier += 1 }
        let (ci, cr) = (creator[id], creator[rep]);
        if ci != u32::MAX && ci == cr { same += 1; }
        else if cr != u32::MAX && (cr as usize) == id { rep_by_child += 1; }
        let g = if rep > id { rep - id } else { id - rep };
        *gap.entry(g.next_power_of_two().trailing_zeros()).or_insert(0usize) += 1;
    }
    println!("delegated={n} unknown={unknown} same_creator={same} rep_created_by_delegated_itself={rep_by_child} rep_later={rep_later} rep_earlier={rep_earlier}");
    println!("id_gap_log2_hist={:?}", gap);
}
