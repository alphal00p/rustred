// Read-only reader of a CP5 walk checkpoint (domains, node flags, edges).
//
// Segment formats follow walking/checkpoint/sections.rs as decoded by the
// W0 census lens (inputs_lens/census.rs): `domains-G.bin` = 32-byte header
// ("RRW5", count at 16..24, first id at 24..32) + varint records; `edges-G.bin`
// = 32-byte header (count at 16..24) + count x (u32 source, u32 target);
// `nodes-G.bin` = 32-byte header + one flag byte per id (bit0 sealed,
// bit1 inspected, bit2 closed).
//
// Usage: cp5hop CHECKPOINT_DIR GEN OWNERS_TXT OUT_DIR [HOP_SOURCES [BOUNDS_TSV]]
//   BOUNDS_TSV: lines "mask rank A" (A = -1 for unbounded): per owner, the
//   Apply domains NOT inside that (rank, A) box are counted in escapes.tsv
// Outputs:
//   hop.tsv        for every edge whose source id < HOP_SOURCES (default 0 =
//                  none): source id, source phase/mask/inspected, target
//                  phase/mask, edge count (aggregated per (source, target key))
//   envelope.tsv   per Apply owner mask: domains, inspected, unbounded-A count,
//                  max finite A upper bound (min(t + sum active upper,
//                  max_positive_power)), unbounded-rank count, max finite rank,
//                  max finite rank among inspected domains, max finite A among
//                  inspected domains
//   transitions.tsv  edge counts by (source phase/mask, target phase/mask)
//   summary.json
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{BufReader, Read, Write};

/// Maximum arity (masks are u16); the actual arity is read from the records.
const NMAX: usize = 16;

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
fn opt(b: &[u8], p: &mut usize) -> Option<u64> {
    let t = b[*p];
    *p += 1;
    match t {
        0 => None,
        1 => Some(varint(b, p)),
        _ => panic!("option tag {t}"),
    }
}

#[derive(Clone, Copy)]
struct Info {
    phase: u8,
    mask: u16,
    rank: Option<u64>,
    a_hi: Option<u64>,
}

fn mask_str(m: u16, n: usize) -> String {
    (0..n).map(|i| if m & (1 << i) != 0 { '1' } else { '0' }).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = &args[1];
    let gen: u64 = args[2].parse().unwrap();
    let owners: Vec<u16> = fs::read_to_string(&args[3])
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.trim().chars().enumerate().fold(0u16, |m, (i, c)| if c == '1' { m | (1 << i) } else { m }))
        .collect();
    let out = &args[4];
    let hop_sources: usize = args.get(5).map(|s| s.parse().unwrap()).unwrap_or(0);
    let bounds: HashMap<u16, (u64, Option<u64>)> = args
        .get(6)
        .map(|p| {
            fs::read_to_string(p)
                .unwrap()
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| {
                    let f: Vec<&str> = l.split_whitespace().collect();
                    let m = f[0].chars().enumerate().fold(0u16, |m, (i, c)| if c == '1' { m | (1 << i) } else { m });
                    let a: i64 = f[2].parse().unwrap();
                    (m, (f[1].parse().unwrap(), if a < 0 { None } else { Some(a as u64) }))
                })
                .collect()
        })
        .unwrap_or_default();
    fs::create_dir_all(out).unwrap();
    let mut infos: Vec<Info> = Vec::new();
    let mut arity = 0usize;
    for g in 0..=gen {
        let path = format!("{dir}/domains-{g:020}.bin");
        let Ok(bytes) = fs::read(&path) else { continue };
        assert_eq!(&bytes[0..4], b"RRW5");
        let count = u64::from_le_bytes(bytes[16..24].try_into().unwrap()) as usize;
        let first = u64::from_le_bytes(bytes[24..32].try_into().unwrap()) as usize;
        assert_eq!(first, infos.len(), "segment {g} first id");
        let mut p = 32usize;
        for _ in 0..count {
            let phase = varint(&bytes, &mut p) as u8;
            let n = varint(&bytes, &mut p) as usize;
            assert!(n <= NMAX && (arity == 0 || arity == n), "arity {n}");
            arity = n;
            let mut mask = 0u16;
            for i in 0..n {
                if bytes[p + i] == 1 {
                    mask |= 1 << i;
                }
            }
            p += n;
            assert_eq!(varint(&bytes, &mut p) as usize, n);
            let mut lower = [0u64; NMAX];
            for l in lower.iter_mut().take(n) {
                *l = varint(&bytes, &mut p);
            }
            assert_eq!(varint(&bytes, &mut p) as usize, n);
            let mut upper = [None; NMAX];
            for u in upper.iter_mut().take(n) {
                *u = opt(&bytes, &mut p);
            }
            let rank = opt(&bytes, &mut p);
            let maxa = opt(&bytes, &mut p);
            let _dmin = opt(&bytes, &mut p);
            let _dmax = opt(&bytes, &mut p);
            let t = mask.count_ones() as u64;
            let mut box_a: Option<u64> = Some(t);
            for i in 0..n {
                if mask & (1 << i) != 0 {
                    box_a = match (box_a, upper[i]) {
                        (Some(a), Some(u)) => Some(a + u),
                        _ => None,
                    };
                }
            }
            let a_hi = match (box_a, maxa) {
                (Some(a), Some(m)) => Some(a.min(m)),
                (Some(a), None) => Some(a),
                (None, m) => m,
            };
            let _ = lower;
            infos.push(Info { phase, mask, rank, a_hi });
        }
        assert_eq!(p, bytes.len(), "segment {g} trailing bytes");
    }
    let total = infos.len();
    let flags = fs::read(format!("{dir}/nodes-{gen:020}.bin")).unwrap()[32..].to_vec();
    assert_eq!(flags.len(), total, "nodes vs domains");
    // ---- one-hop edges
    let mut hop: BTreeMap<(usize, u8, u16), u64> = BTreeMap::new();
    let mut trans: HashMap<u64, u64> = HashMap::new();
    let mut edges_total = 0u64;
    for g in 0..=gen {
        let path = format!("{dir}/edges-{g:020}.bin");
        let Ok(f) = fs::File::open(&path) else { continue };
        let mut f = BufReader::with_capacity(1 << 24, f);
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
                let (a, b) = (&infos[s], &infos[t]);
                *trans.entry(((a.phase as u64) << 48) | ((a.mask as u64) << 32) | ((b.phase as u64) << 16) | b.mask as u64).or_default() += 1;
                if s < hop_sources {
                    *hop.entry((s, infos[t].phase, infos[t].mask)).or_default() += 1;
                }
            }
            left -= n;
        }
        edges_total += count;
    }
    let mut w = std::io::BufWriter::new(fs::File::create(format!("{out}/transitions.tsv")).unwrap());
    writeln!(w, "source_phase\tsource_mask\ttarget_phase\ttarget_mask\tedges").unwrap();
    let mut tk: Vec<_> = trans.iter().collect();
    tk.sort();
    let ph = |p: u64| if p == 0 { "Apply" } else { "Route" };
    for (k, c) in tk {
        writeln!(w, "{}\t{}\t{}\t{}\t{}", ph(k >> 48), mask_str(((k >> 32) & 0xffff) as u16, arity), ph((k >> 16) & 0xffff), mask_str((k & 0xffff) as u16, arity), c).unwrap();
    }
    drop(w);
    let mut w = std::io::BufWriter::new(fs::File::create(format!("{out}/hop.tsv")).unwrap());
    writeln!(w, "source\tsource_phase\tsource_mask\tsource_inspected\ttarget_phase\ttarget_mask\ttarget_is_owner\tedges").unwrap();
    for ((s, ph, m), c) in &hop {
        let si = infos[*s];
        writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", s, if si.phase == 0 { "Apply" } else { "Route" }, mask_str(si.mask, arity),
            (flags[*s] >> 1) & 1, if *ph == 0 { "Apply" } else { "Route" }, mask_str(*m, arity), owners.contains(m) as u8, c).unwrap();
    }
    // ---- per-owner Apply envelope
    #[derive(Default)]
    struct Env {
        n: u64,
        insp: u64,
        a_unb: u64,
        a_max: u64,
        r_unb: u64,
        r_max: u64,
        r_max_insp: u64,
        a_max_insp: u64,
    }
    let mut env: HashMap<u16, Env> = HashMap::new();
    for (id, i) in infos.iter().enumerate() {
        if i.phase != 0 {
            continue;
        }
        let e = env.entry(i.mask).or_default();
        let inspected = (flags[id] >> 1) & 1 == 1;
        e.n += 1;
        e.insp += inspected as u64;
        match i.a_hi {
            None => e.a_unb += 1,
            Some(a) => {
                e.a_max = e.a_max.max(a);
                if inspected {
                    e.a_max_insp = e.a_max_insp.max(a);
                }
            }
        }
        match i.rank {
            None => e.r_unb += 1,
            Some(r) => {
                e.r_max = e.r_max.max(r);
                if inspected {
                    e.r_max_insp = e.r_max_insp.max(r);
                }
            }
        }
    }
    let mut w = std::io::BufWriter::new(fs::File::create(format!("{out}/envelope.tsv")).unwrap());
    writeln!(w, "mask\towner_index\tapply_domains\tinspected\tunbounded_A\tmax_finite_A\tunbounded_rank\tmax_finite_rank\tmax_rank_inspected\tmax_A_inspected").unwrap();
    let mut keys: Vec<_> = env.keys().copied().collect();
    keys.sort_by_key(|m| owners.iter().position(|o| o == m).unwrap_or(usize::MAX));
    for m in keys {
        let e = &env[&m];
        let idx = owners.iter().position(|o| *o == m).map(|i| i.to_string()).unwrap_or("-".into());
        writeln!(w, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", mask_str(m, arity), idx, e.n, e.insp, e.a_unb, e.a_max, e.r_unb, e.r_max, e.r_max_insp, e.a_max_insp).unwrap();
    }
    if !bounds.is_empty() {
        // escapes: Apply domains of a bounded owner outside its (rank, A) box
        let mut esc: BTreeMap<u16, [u64; 6]> = BTreeMap::new(); // domains, escapes, by rank, by A, unbounded A, inspected escapes
        for (id, i) in infos.iter().enumerate() {
            if i.phase != 0 {
                continue;
            }
            let Some(&(r_h, a_h)) = bounds.get(&i.mask) else { continue };
            let e = esc.entry(i.mask).or_default();
            e[0] += 1;
            let by_rank = i.rank.is_none_or(|r| r > r_h);
            let by_a = match (a_h, i.a_hi) {
                (None, _) => false,
                (Some(_), None) => true,
                (Some(h), Some(a)) => a > h,
            };
            if by_rank || by_a {
                e[1] += 1;
                e[5] += ((flags[id] >> 1) & 1) as u64;
            }
            e[2] += by_rank as u64;
            e[3] += by_a as u64;
            e[4] += i.a_hi.is_none() as u64;
        }
        let mut w = std::io::BufWriter::new(fs::File::create(format!("{out}/escapes.tsv")).unwrap());
        writeln!(w, "mask	helper_rank	helper_A	apply_domains	escapes	escapes_by_rank	escapes_by_A	unbounded_A	inspected_escapes").unwrap();
        for (m, e) in &esc {
            let (r_h, a_h) = bounds[m];
            writeln!(w, "{}	{}	{}	{}	{}	{}	{}	{}	{}", mask_str(*m, arity), r_h, a_h.map(|a| a as i64).unwrap_or(-1), e[0], e[1], e[2], e[3], e[4], e[5]).unwrap();
        }
    }
    // phase census: domains and inspected domains per phase
    let mut by_phase = [[0u64; 2]; 2];
    for (id, i) in infos.iter().enumerate() {
        let p = (i.phase as usize).min(1);
        by_phase[p][0] += 1;
        by_phase[p][1] += ((flags[id] >> 1) & 1) as u64;
    }
    let mut w = fs::File::create(format!("{out}/summary.json")).unwrap();
    writeln!(w, "{{\"checkpoint\":\"{dir}\",\"generation\":{gen},\"domains\":{total},\"edges\":{edges_total},\"hop_sources\":{hop_sources},\"hop_rows\":{},\"apply_domains\":{},\"apply_inspected\":{},\"route_domains\":{},\"route_inspected\":{}}}",
        hop.len(), by_phase[0][0], by_phase[0][1], by_phase[1][0], by_phase[1][1]).unwrap();
}
