// Box-volume census of CP5 domain segments: singleton / finite / unbounded axes.
use std::collections::HashMap;
use std::fs;
const N: usize = 15;
fn varint(b: &[u8], p: &mut usize) -> u64 { let x = b[*p]; *p += 1; match x { 0..=250 => x as u64, 251 => { let v = u16::from_le_bytes(b[*p..*p+2].try_into().unwrap()) as u64; *p += 2; v } 252 => { let v = u32::from_le_bytes(b[*p..*p+4].try_into().unwrap()) as u64; *p += 4; v } 253 => { let v = u64::from_le_bytes(b[*p..*p+8].try_into().unwrap()); *p += 8; v } _ => panic!() } }
fn opt(b: &[u8], p: &mut usize) -> Option<u64> { let t = b[*p]; *p += 1; if t == 0 { None } else { Some(varint(b, p)) } }
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let dir = &a[1]; let gen: u64 = a[2].parse().unwrap();
    let flags = fs::read(format!("{dir}/nodes-{gen:020}.bin")).unwrap();
    let flags = &flags[32..];
    // key: (phase, status, unbounded_axes(0..15), log2vol bucket (finite only; 99 if unbounded), singleton) -> count
    let mut h: HashMap<(u8, u8, u8, u8, u8, u8, u8), u64> = HashMap::new();
    let mut id = 0usize;
    for g in 0..=gen {
        let Ok(bytes) = fs::read(format!("{dir}/domains-{g:020}.bin")) else { continue };
        let count = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        let mut p = 32usize;
        for _ in 0..count {
            let phase = varint(&bytes, &mut p) as u8;
            let _ = varint(&bytes, &mut p); let mut mask = 0u16; for i in 0..N { if bytes[p+i]==1 { mask |= 1<<i } } p += N;
            let _ = varint(&bytes, &mut p); let mut lo = [0u64; N]; for i in 0..N { lo[i] = varint(&bytes, &mut p); }
            let _ = varint(&bytes, &mut p); let mut up = [None; N]; for i in 0..N { up[i] = opt(&bytes, &mut p); }
            let rank = opt(&bytes, &mut p); let maxa = opt(&bytes, &mut p); let _ = opt(&bytes, &mut p); let _ = opt(&bytes, &mut p);
            // effective per-axis extents: active axes capped by maxa, inactive by rank
            let t = mask.count_ones() as u64;
            let sum_lo_act: u64 = (0..N).filter(|i| mask & (1<<i) != 0).map(|i| lo[i]).sum();
            let sum_lo_in: u64 = (0..N).filter(|i| mask & (1<<i) == 0).map(|i| lo[i]).sum();
            let mut unb = 0u8; let mut vol: f64 = 1.0; let mut single = true; let mut nontriv = 0u8;
            for i in 0..N {
                let act = mask & (1<<i) != 0;
                let cap = if act { maxa.map(|m| m.saturating_sub(t + sum_lo_act) + lo[i]) } else { rank.map(|r| r.saturating_sub(sum_lo_in) + lo[i]) };
                let hi = match (up[i], cap) { (Some(u), Some(c)) => Some(u.min(c)), (Some(u), None) => Some(u), (None, c) => c };
                match hi { None => { unb += 1; single = false; } Some(hv) => { let w = hv.saturating_sub(lo[i]) + 1; if w > 1 { single = false; nontriv += 1; } vol *= w as f64; } }
            }
            let lv = if unb > 0 { 99u8 } else { vol.log2().floor() as u8 };
            let f = flags[id]; let st = if f & 2 != 0 { 1 } else if f & 1 != 0 { 2 } else { 0 };
            *h.entry((phase, st, unb, lv, single as u8, nontriv, (rank.is_none()) as u8)).or_default() += 1;
            id += 1;
        }
    }
    let mut k: Vec<_> = h.into_iter().collect(); k.sort();
    println!("phase\tstatus\tunbounded_axes\tlog2vol\tsingleton\tnontrivial_axes\trank_none\tcount");
    for (k, v) in k { println!("{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", k.0, k.1, k.2, k.3, k.4, k.5, k.6, v); }
}
