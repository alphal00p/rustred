// Read-only census of a CP5 walk checkpoint (domains, node flags, edges).
// Usage: census CHECKPOINT_DIR GEN OWNERS_TSV OUT_DIR [bfs]
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, Read, Write};
use std::time::Instant;

const N: usize = 15;

fn varint(b: &[u8], p: &mut usize) -> u64 {
    let x = b[*p];
    *p += 1;
    match x {
        0..=250 => x as u64,
        251 => {
            let v = u16::from_le_bytes(b[*p..*p + 2].try_into().unwrap()) as u64;
            *p += 2;
            v
        }
        252 => {
            let v = u32::from_le_bytes(b[*p..*p + 4].try_into().unwrap()) as u64;
            *p += 4;
            v
        }
        253 => {
            let v = u64::from_le_bytes(b[*p..*p + 8].try_into().unwrap());
            *p += 8;
            v
        }
        _ => panic!("varint tag {x}"),
    }
}
fn zz(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}
fn opt(b: &[u8], p: &mut usize) -> Option<u64> {
    let t = b[*p];
    *p += 1;
    match t {
        0 => None,
        1 => Some(varint(b, p)),
        _ => panic!("option tag {t}"),
    }
}

#[derive(Clone, Copy, Default)]
struct Info {
    phase: u8,  // 0 apply 1 route
    mask: u16,  // bit i = owner[i]
    owner: u8,  // 0..66 or 255
    rank: u8,   // 255 none
    a_lo: u8,
    a_hi: u8,   // 255 unbounded
    r_lo: u8,
    d_lo: i8,   // -128 = -inf
    d_hi: i8,   // 127 = +inf
    in_v2_helper: bool,
    in_int_helper: bool,
}

struct Owner {
    mask: u16,
    class: String,
    t: u32,
    r_h: u32,
    a_h: i64,
    r_hi: u32,
    a_hi: i64,
    guard: bool,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let gen: u64 = args[2].parse().unwrap();
    let owners_tsv = &args[3];
    let out = &args[4];
    let do_bfs = args.get(5).map(|s| s == "bfs").unwrap_or(false);
    let t0 = Instant::now();
    let mut owners: Vec<Owner> = Vec::new();
    for line in fs::read_to_string(owners_tsv).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let mask = f[1].chars().enumerate().fold(0u16, |m, (i, c)| if c == '1' { m | (1 << i) } else { m });
        owners.push(Owner {
            mask,
            class: f[2].to_string(),
            t: f[3].parse().unwrap(),
            r_h: f[5].parse().unwrap(),
            a_h: f[6].parse().unwrap(),
            r_hi: f[7].parse().unwrap(),
            a_hi: f[8].parse().unwrap(),
            guard: f[9] == "1",
        });
    }
    let owner_of: HashMap<u16, u8> = owners.iter().enumerate().map(|(i, o)| (o.mask, i as u8)).collect();

    // ---- domains
    let mut infos: Vec<Info> = Vec::new();
    for g in 0..=gen {
        let path = format!("{dir}/domains-{g:020}.bin");
        let Ok(bytes) = fs::read(&path) else { continue };
        let count = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        let first = u64::from_le_bytes(bytes[24..32].try_into().unwrap()) as usize;
        assert_eq!(&bytes[0..4], b"RRW5");
        assert_eq!(first, infos.len(), "segment {g} first");
        let mut p = 32usize;
        for _ in 0..count {
            let phase = varint(&bytes, &mut p) as u8;
            let n = varint(&bytes, &mut p) as usize;
            assert_eq!(n, N);
            let mut mask = 0u16;
            for i in 0..N {
                if bytes[p + i] == 1 {
                    mask |= 1 << i;
                }
            }
            p += N;
            let n = varint(&bytes, &mut p) as usize;
            assert_eq!(n, N);
            let mut lower = [0u64; N];
            for i in 0..N {
                lower[i] = varint(&bytes, &mut p);
            }
            let n = varint(&bytes, &mut p) as usize;
            assert_eq!(n, N);
            let mut upper = [None; N];
            for i in 0..N {
                upper[i] = opt(&bytes, &mut p);
            }
            let rank = opt(&bytes, &mut p);
            let maxa = opt(&bytes, &mut p);
            let dmin = opt(&bytes, &mut p).map(zz);
            let dmax = opt(&bytes, &mut p).map(zz);
            let t = mask.count_ones() as u64;
            let mut a_lo = t;
            let mut a_up: Option<u64> = Some(t);
            let mut r_lo = 0u64;
            let mut r_up: Option<u64> = Some(0);
            for i in 0..N {
                if mask & (1 << i) != 0 {
                    a_lo += lower[i];
                    a_up = match (a_up, upper[i]) {
                        (Some(a), Some(u)) => Some(a + u + 0),
                        _ => None,
                    };
                } else {
                    r_lo += lower[i];
                    r_up = match (r_up, upper[i]) {
                        (Some(a), Some(u)) => Some(a + u),
                        _ => None,
                    };
                }
            }
            // a_up above counted t + sum(upper) (x = n - 1).
            let a_hi = match (a_up, maxa) {
                (Some(a), Some(m)) => Some(a.min(m)),
                (Some(a), None) => Some(a),
                (None, m) => m,
            };
            let r_hi = match (r_up, rank) {
                (Some(a), Some(r)) => Some(a.min(r)),
                (Some(a), None) => Some(a),
                (None, r) => r,
            };
            let d_lo = {
                let base = match r_hi { Some(r) => Some(a_lo as i64 - r as i64), None => None };
                match (base, dmin) {
                    (Some(b), Some(d)) => Some(b.max(d)),
                    (Some(b), None) => Some(b),
                    (None, d) => d,
                }
            };
            let d_hi = {
                let base = a_hi.map(|a| a as i64 - r_lo as i64);
                match (base, dmax) {
                    (Some(b), Some(d)) => Some(b.min(d)),
                    (Some(b), None) => Some(b),
                    (None, d) => d,
                }
            };
            let owner = if phase == 0 { *owner_of.get(&mask).unwrap_or(&255) } else { 255 };
            let mut info = Info {
                phase,
                mask,
                owner,
                rank: rank.map(|r| r.min(254) as u8).unwrap_or(255),
                a_lo: a_lo.min(254) as u8,
                a_hi: a_hi.map(|a| a.min(254) as u8).unwrap_or(255),
                r_lo: r_lo.min(254) as u8,
                d_lo: d_lo.map(|d| d.clamp(-127, 126) as i8).unwrap_or(-128),
                d_hi: d_hi.map(|d| d.clamp(-127, 126) as i8).unwrap_or(127),
                in_v2_helper: false,
                in_int_helper: false,
            };
            if owner != 255 {
                let o = &owners[owner as usize];
                let rank_ok = |h: u32| rank.is_some_and(|r| r <= h as u64);
                let a_ok = |h: i64| h < 0 || maxa.is_some_and(|m| m as i64 <= h);
                info.in_v2_helper = rank_ok(o.r_h) && a_ok(o.a_h);
                info.in_int_helper = rank_ok(o.r_hi) && a_ok(o.a_hi);
            }
            infos.push(info);
        }
        assert_eq!(p, bytes.len(), "segment {g} trailing");
        eprintln!("segment {g}: {count} domains, total {} at {:.1}s", infos.len(), t0.elapsed().as_secs_f64());
    }
    let total = infos.len();
    let flags = {
        let bytes = fs::read(format!("{dir}/nodes-{gen:020}.bin")).unwrap();
        bytes[32..].to_vec()
    };
    assert_eq!(flags.len(), total, "nodes vs domains");
    eprintln!("nodes read {:.1}s", t0.elapsed().as_secs_f64());

    // ---- per-domain census
    let mut w = fs::File::create(format!("{out}/census.tsv")).unwrap();
    // status: 0 open (unsealed, not inspected), 1 inspected, 2 sealed not inspected
    let status = |f: u8| -> usize {
        if f & 2 != 0 { 1 } else if f & 1 != 0 { 2 } else { 0 }
    };
    // (phase, owner-or-255, rank, a_hi, d_lo_bucket, v2helper, inthelper, status, closed) -> count
    let mut hist: HashMap<(u8, u8, u8, u8, u8, i8, i8, bool, bool, usize, bool), u64> = HashMap::new();
    let mut route_t: HashMap<(u32, usize), u64> = HashMap::new();
    let mut route_masks: HashMap<u16, u64> = HashMap::new();
    for (id, info) in infos.iter().enumerate() {
        let f = flags[id];
        let st = status(f);
        let closed = f & 4 != 0;
        if info.phase == 1 {
            *route_t.entry((info.mask.count_ones(), st)).or_default() += 1;
            *route_masks.entry(info.mask).or_default() += 1;
        }
        let key = (
            info.phase,
            info.owner,
            if info.phase == 1 { info.mask.count_ones() as u8 } else { 0 },
            info.rank,
            info.a_hi,
            if (info.d_hi as i32) < 9 { 0i8 } else if (info.d_lo as i32) > 10 { 2 } else { 1 },
            info.d_lo.max(-20).min(30),
            info.in_v2_helper,
            info.in_int_helper,
            st,
            closed,
        );
        *hist.entry(key).or_default() += 1;
    }
    writeln!(w, "phase\towner\troute_t\trank\ta_hi\tdclass(0<9,1meets9-10,2>10)\td_lo\tin_v2_helper\tin_int_helper\tstatus\tclosed\tcount").unwrap();
    let mut keys: Vec<_> = hist.iter().collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    for (k, v) in keys {
        writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", k.0, k.1, k.2, k.3, k.4, k.5, k.6, k.7 as u8, k.8 as u8, k.9, k.10 as u8, v).unwrap();
    }
    eprintln!("census written {:.1}s; route masks {}", t0.elapsed().as_secs_f64(), route_masks.len());
    // a_lo / r_lo histogram of apply domains per owner (for pending vs inspected)
    let mut w2 = fs::File::create(format!("{out}/apply_lo.tsv")).unwrap();
    let mut h2: HashMap<(u8, u8, u8, usize), u64> = HashMap::new();
    for (id, info) in infos.iter().enumerate() {
        if info.phase == 0 {
            *h2.entry((info.owner, info.a_lo, info.r_lo, status(flags[id]))).or_default() += 1;
        }
    }
    let mut k2: Vec<_> = h2.iter().collect();
    k2.sort_by(|a, b| a.0.cmp(b.0));
    writeln!(w2, "owner\ta_lo\tr_lo\tstatus\tcount").unwrap();
    for (k, v) in k2 {
        writeln!(w2, "{}\t{}\t{}\t{}\t{}", k.0, k.1, k.2, k.3, v).unwrap();
    }
    // initial ids check
    let mut w3 = fs::File::create(format!("{out}/initial.tsv")).unwrap();
    for id in 0..owners.len().min(total) {
        let i = infos[id];
        writeln!(w3, "{id}\t{}\t{}\t{}\t{}\t{}\t{}", i.phase, i.owner, i.rank, i.a_hi, flags[id], i.in_v2_helper as u8).unwrap();
    }

    // ---- edges: owner-level transition counts (Apply owner idx, or Route sector mask as 1000+mask)
    let node_key = |i: &Info| -> u32 { if i.phase == 0 { if i.owner != 255 { i.owner as u32 } else { 100000 + i.mask as u32 } } else { 200000 + i.mask as u32 } };
    let mut trans: HashMap<(u32, u32), u64> = HashMap::new();
    let mut edge_files: Vec<String> = Vec::new();
    for g in 0..=gen {
        let path = format!("{dir}/edges-{g:020}.bin");
        if fs::metadata(&path).is_ok() {
            edge_files.push(path);
        }
    }
    let mut out_deg: Vec<u32> = if do_bfs { vec![0u32; total] } else { Vec::new() };
    let mut edges_total = 0u64;
    let mut self_owner_edges = 0u64;
    for path in &edge_files {
        let mut f = BufReader::with_capacity(1 << 24, fs::File::open(path).unwrap());
        let mut head = [0u8; 32];
        f.read_exact(&mut head).unwrap();
        let count = u64::from_le_bytes(head[16..24].try_into().unwrap());
        let mut buf = vec![0u8; 8 << 20];
        let mut left = count as usize * 8;
        while left > 0 {
            let n = left.min(buf.len());
            f.read_exact(&mut buf[..n]).unwrap();
            for c in buf[..n].chunks_exact(8) {
                let s = u32::from_le_bytes(c[0..4].try_into().unwrap()) as usize;
                let t = u32::from_le_bytes(c[4..8].try_into().unwrap()) as usize;
                let ks = node_key(&infos[s]);
                let kt = node_key(&infos[t]);
                if ks == kt { self_owner_edges += 1; }
                *trans.entry((ks, kt)).or_default() += 1;
                if do_bfs { out_deg[s] += 1; }
            }
            left -= n;
        }
        edges_total += count;
        eprintln!("edges {path}: {count} at {:.1}s", t0.elapsed().as_secs_f64());
    }
    let mut w4 = fs::File::create(format!("{out}/transitions.tsv")).unwrap();
    writeln!(w4, "source\ttarget\tcount").unwrap();
    let mut tk: Vec<_> = trans.iter().collect();
    tk.sort();
    for (k, v) in tk {
        writeln!(w4, "{}\t{}\t{}", k.0, k.1, v).unwrap();
    }
    eprintln!("edges total {edges_total}, same-key {self_owner_edges}, transition pairs {} at {:.1}s", trans.len(), t0.elapsed().as_secs_f64());
    if !do_bfs {
        return;
    }
    // ---- CSR forward
    let mut offs: Vec<u64> = vec![0; total + 1];
    for i in 0..total {
        offs[i + 1] = offs[i] + out_deg[i] as u64;
    }
    drop(out_deg);
    let mut targets: Vec<u32> = vec![0; edges_total as usize];
    let mut fill: Vec<u64> = offs[..total].to_vec();
    for path in &edge_files {
        let mut f = BufReader::with_capacity(1 << 24, fs::File::open(path).unwrap());
        let mut head = [0u8; 32];
        f.read_exact(&mut head).unwrap();
        let count = u64::from_le_bytes(head[16..24].try_into().unwrap());
        let mut buf = vec![0u8; 8 << 20];
        let mut left = count as usize * 8;
        while left > 0 {
            let n = left.min(buf.len());
            f.read_exact(&mut buf[..n]).unwrap();
            for c in buf[..n].chunks_exact(8) {
                let s = u32::from_le_bytes(c[0..4].try_into().unwrap()) as usize;
                let t = u32::from_le_bytes(c[4..8].try_into().unwrap());
                targets[fill[s] as usize] = t;
                fill[s] += 1;
            }
            left -= n;
        }
    }
    drop(fill);
    eprintln!("CSR built at {:.1}s", t0.elapsed().as_secs_f64());
    // ---- per-root BFS in parallel, bitsets kept for attribution
    let roots = owners.len();
    let words = (total + 63) / 64;
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results: std::sync::Mutex<Vec<Option<Vec<u64>>>> = std::sync::Mutex::new(vec![None; roots]);
    let threads: usize = std::env::var("CENSUS_THREADS").ok().and_then(|s| s.parse().ok()).unwrap_or(12);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| {
                let mut stack: Vec<u32> = Vec::new();
                loop {
                    let r = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if r >= roots { break; }
                    let mut vis = vec![0u64; words];
                    stack.clear();
                    stack.push(r as u32);
                    vis[r / 64] |= 1 << (r % 64);
                    while let Some(v) = stack.pop() {
                        let v = v as usize;
                        for e in offs[v]..offs[v + 1] {
                            let t = targets[e as usize] as usize;
                            let (wi, bi) = (t / 64, 1u64 << (t % 64));
                            if vis[wi] & bi == 0 {
                                vis[wi] |= bi;
                                stack.push(t as u32);
                            }
                        }
                    }
                    eprintln!("root {r} done at {:.1}s", t0.elapsed().as_secs_f64());
                    results.lock().unwrap()[r] = Some(vis);
                }
            });
        }
    });
    let results: Vec<Vec<u64>> = results.into_inner().unwrap().into_iter().map(|v| v.unwrap()).collect();
    // per node: number of roots reaching it (saturating u8) and class bits
    let class_bit = |c: &str| -> u8 { match c { "connected" => 1, "factorized" => 2, _ => 4 } };
    let mut cnt: Vec<u8> = vec![0; total];
    let mut cls: Vec<u8> = vec![0; total];
    for (r, vis) in results.iter().enumerate() {
        let b = class_bit(&owners[r].class) | if owners[r].guard { 8 } else { 0 };
        for (wi, &word) in vis.iter().enumerate() {
            let mut x = word;
            while x != 0 {
                let bit = x.trailing_zeros() as usize;
                x &= x - 1;
                let v = wi * 64 + bit;
                cnt[v] = cnt[v].saturating_add(1);
                cls[v] |= b;
            }
        }
    }
    let mut w5 = fs::File::create(format!("{out}/root_reach.tsv")).unwrap();
    writeln!(w5, "root\treach\treach_inspected\treach_open\treach_sealed_only\texcl\texcl_inspected\texcl_open\treach_apply_outside_own_helper").unwrap();
    for (r, vis) in results.iter().enumerate() {
        let (mut n, mut ni, mut no, mut ns, mut ex, mut exi, mut exo, mut nout) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
        for (wi, &word) in vis.iter().enumerate() {
            let mut x = word;
            while x != 0 {
                let bit = x.trailing_zeros() as usize;
                x &= x - 1;
                let v = wi * 64 + bit;
                n += 1;
                let st = status(flags[v]);
                match st { 1 => ni += 1, 0 => no += 1, _ => ns += 1 }
                if cnt[v] == 1 { ex += 1; if st == 1 { exi += 1 } else if st == 0 { exo += 1 } }
                if infos[v].phase == 0 && infos[v].owner != 255 && !infos[v].in_v2_helper { nout += 1; }
            }
        }
        writeln!(w5, "{r}\t{n}\t{ni}\t{no}\t{ns}\t{ex}\t{exi}\t{exo}\t{nout}").unwrap();
    }
    let mut ch: HashMap<(u8, usize, u8, u8), u64> = HashMap::new();
    for v in 0..total {
        *ch.entry((cls[v], status(flags[v]), infos[v].phase, cnt[v].min(9))).or_default() += 1;
    }
    let mut w7 = fs::File::create(format!("{out}/class_union.tsv")).unwrap();
    writeln!(w7, "classbits(1=conn,2=fact,4=nonentry,8=guardowner)\tstatus\tphase\troots_reaching(min9)\tcount").unwrap();
    let mut chk: Vec<_> = ch.iter().collect();
    chk.sort();
    for (k, v) in chk {
        writeln!(w7, "{}\t{}\t{}\t{}\t{}", k.0, k.1, k.2, k.3, v).unwrap();
    }
    eprintln!("done at {:.1}s", t0.elapsed().as_secs_f64());
}
