// Counterfactual reach: Apply nodes in "absorbed" owners are sinks (aliased to an unrestricted orthant).
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read};
const N: usize = 15;
fn varint(b: &[u8], p: &mut usize) -> u64 { let x = b[*p]; *p += 1; match x { 0..=250 => x as u64, 251 => { let v = u16::from_le_bytes(b[*p..*p+2].try_into().unwrap()) as u64; *p += 2; v } 252 => { let v = u32::from_le_bytes(b[*p..*p+4].try_into().unwrap()) as u64; *p += 4; v } 253 => { let v = u64::from_le_bytes(b[*p..*p+8].try_into().unwrap()); *p += 8; v } _ => panic!() } }
fn opt(b: &[u8], p: &mut usize) -> Option<u64> { let t = b[*p]; *p += 1; if t == 0 { None } else { Some(varint(b, p)) } }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1]; let gen: u64 = a[2].parse().unwrap();
    let owners: Vec<(u16, String, u8, i64)> = fs::read_to_string(&a[3]).unwrap().lines().map(|l| { let f: Vec<&str> = l.split('\t').collect(); (f[1].chars().enumerate().fold(0u16, |m, (i, c)| if c == '1' { m | (1 << i) } else { m }), f[2].to_string(), f[5].parse::<u8>().unwrap(), f[6].parse::<i64>().unwrap()) }).collect();
    let owner_of: HashMap<u16, u8> = owners.iter().enumerate().map(|(i, o)| (o.0, i as u8)).collect();
    // scenarios: each arg after 4 is "name:comma-list-of-absorbed-owner-ids"
    let scenarios: Vec<(String, Vec<bool>)> = a[4..].iter().map(|s| { let (n, l) = s.split_once(':').unwrap(); let mut v = vec![false; owners.len()]; for x in l.split(',').filter(|x| !x.is_empty()) { v[x.parse::<usize>().unwrap()] = true; } (n.to_string(), v) }).collect();
    let flags = fs::read(format!("{dir}/nodes-{gen:020}.bin")).unwrap(); let flags = &flags[32..];
    let mut node: Vec<u8> = Vec::new(); let mut rk: Vec<u8> = Vec::new(); let mut ma: Vec<u8> = Vec::new(); // bit7 = route; low 7 bits owner idx (127 = none)
    for g in 0..=gen {
        let Ok(bytes) = fs::read(format!("{dir}/domains-{g:020}.bin")) else { continue };
        let count = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        let mut p = 32usize;
        for _ in 0..count {
            let phase = varint(&bytes, &mut p) as u8;
            let _ = varint(&bytes, &mut p); let mut mask = 0u16; for i in 0..N { if bytes[p+i]==1 { mask |= 1<<i } } p += N;
            let _ = varint(&bytes, &mut p); for _ in 0..N { varint(&bytes, &mut p); }
            let _ = varint(&bytes, &mut p); for _ in 0..N { opt(&bytes, &mut p); }
            let r = opt(&bytes, &mut p); let mx = opt(&bytes, &mut p); for _ in 0..2 { opt(&bytes, &mut p); }
            ma.push(mx.map(|x| x.min(254) as u8).unwrap_or(255));
            rk.push(r.map(|x| x.min(254) as u8).unwrap_or(255));
            let o = if phase == 0 { *owner_of.get(&mask).unwrap_or(&127) } else { 127 };
            node.push(if phase == 1 { 128 | 127 } else { o });
        }
    }
    let total = node.len();
    let mut files = vec![]; for g in 0..=gen { let p = format!("{dir}/edges-{g:020}.bin"); if fs::metadata(&p).is_ok() { files.push(p); } }
    let read_edges = |f: &mut dyn FnMut(usize, u32)| { for path in &files { let mut r = BufReader::with_capacity(1<<24, fs::File::open(path).unwrap()); let mut h = [0u8; 32]; r.read_exact(&mut h).unwrap(); let count = u64::from_le_bytes(h[16..24].try_into().unwrap()) as usize; let mut buf = vec![0u8; 8<<20]; let mut left = count*8; while left > 0 { let n = left.min(buf.len()); r.read_exact(&mut buf[..n]).unwrap(); for c in buf[..n].chunks_exact(8) { f(u32::from_le_bytes(c[0..4].try_into().unwrap()) as usize, u32::from_le_bytes(c[4..8].try_into().unwrap())); } left -= n; } } };
    let mut deg = vec![0u32; total]; let mut m = 0usize; read_edges(&mut |s, _| { deg[s] += 1; m += 1; });
    let mut offs = vec![0u64; total + 1]; for i in 0..total { offs[i+1] = offs[i] + deg[i] as u64; } drop(deg);
    let mut tg = vec![0u32; m]; let mut fill: Vec<u64> = offs[..total].to_vec(); read_edges(&mut |s, t| { tg[fill[s] as usize] = t; fill[s] += 1; }); drop(fill);
    eprintln!("graph: {total} nodes, {m} edges");
    let roots = owners.len();
    for (name, absorbed) in &scenarios {
        let rank_only = name.starts_with("R_");
        let helper_only = name.starts_with("H_");
        let a_only = name.starts_with("A_");
        let sink = |v: usize| -> bool { if helper_only { return v < roots && absorbed[v]; } let k = node[v];
            if a_only && k & 128 == 0 && k != 127 && !absorbed[k as usize] { let ah = owners[k as usize].3; return v >= roots && (ah < 0 || (ma[v] != 255 && (ma[v] as i64) <= ah)); } k & 128 == 0 && k != 127 && absorbed[k as usize] && (!rank_only || rk[v] <= owners[k as usize].2) };
        let mut vis = vec![0u64; (total + 63) / 64]; let mut st: Vec<u32> = (0..roots as u32).filter(|&r| !(helper_only && absorbed[r as usize])).collect();
        for &r in &st { let r = r as usize; vis[r/64] |= 1 << (r%64); }
        while let Some(v) = st.pop() { let v = v as usize; if sink(v) { continue; } for e in offs[v]..offs[v+1] { let t = tg[e as usize] as usize; if vis[t/64] & (1 << (t%64)) == 0 { vis[t/64] |= 1 << (t%64); st.push(t as u32); } } }
        let mut c: HashMap<(&str, &str, u8), u64> = HashMap::new();
        for v in 0..total { if vis[v/64] & (1 << (v%64)) == 0 { continue; } let k = node[v]; let kind = if k & 128 != 0 { "route" } else if absorbed.get(k as usize).copied().unwrap_or(false) { "apply_absorbed_sink" } else { "apply_kept" }; let f = flags[v]; let st = if f & 2 != 0 { 1 } else if f & 1 != 0 { 2 } else { 0 }; *c.entry((kind, if st == 1 { "inspected" } else if st == 0 { "open" } else { "sealed_only" }, 0)).or_default() += 1; }
        let mut k: Vec<_> = c.into_iter().collect(); k.sort();
        let tot: u64 = k.iter().map(|x| x.1).sum();
        println!("scenario {name}: reachable {tot}");
        for ((a, b, _), v) in k { println!("  {a}\t{b}\t{v}"); }
    }
}
