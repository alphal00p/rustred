//! `lag` mode: container age of every committed dependency edge of a CP5
//! checkpoint, in admission (ID) units. Edges are appended in commit order and
//! deduplicated per source; a target above the running maximum ID is the
//! creation edge of a new domain, any other edge is a hit on an existing
//! container whose minimum-ID choice makes its age exact: with a snapshot
//! lagging by L admissions, that request finds no container iff age < L.
//! Self edges (self-scope tier), targets created by the same source earlier
//! (Local tier) and helper targets (< initial count) are separated out.
use crate::Args;
use crate::ckpt::{self, header};
use crate::stat::Out;
use crate::util::Json;
use std::io::Read;
use std::path::Path;

const BOUNDS: [u64; 12] = [0, 1, 64, 1024, 4096, 16384, 65536, 262_144, 1_048_576, 4_194_304, 16_777_216, u64::MAX];

pub fn run(args: &Args) {
    let dir = Path::new(args.req("ckpt"));
    let initial = args.num("initial", 67) as u32;
    let label = args.get("label").unwrap_or("").to_string();
    let mut out = Out::open(args.get("out").unwrap_or("idxreplay-lag.jsonl"));
    let segs = ckpt::edge_segments(dir);
    let mut creator: Vec<u32> = Vec::new();
    let mut m: u64 = initial as u64 - 1;
    let mut hist = [0u64; 11];
    let (mut edges, mut creation, mut selfe, mut local, mut helper) = (0u64, 0u64, 0u64, 0u64, 0u64);
    // Per-segment histograms too (early vs late campaign).
    for p in &segs {
        let mut f = std::fs::File::open(p).unwrap();
        let mut hdr = [0u8; 32];
        f.read_exact(&mut hdr).unwrap();
        let (count, first) = header(&hdr);
        let mut seg_hist = [0u64; 11];
        let mut buf = vec![0u8; 8 << 20];
        let mut left = count as usize * 8;
        let mut carry: Vec<u8> = Vec::new();
        while left > 0 {
            let n = f.read(&mut buf[..left.min(8 << 20)]).unwrap();
            assert!(n > 0, "short edge segment");
            left -= n;
            carry.extend_from_slice(&buf[..n]);
            let whole = carry.len() / 8 * 8;
            for e in carry[..whole].chunks_exact(8) {
                let s = u32::from_le_bytes(e[0..4].try_into().unwrap());
                let t = u32::from_le_bytes(e[4..8].try_into().unwrap());
                edges += 1;
                if t as u64 > m {
                    m = t as u64;
                    creation += 1;
                    if creator.len() <= t as usize {
                        creator.resize(t as usize + 1 + (1 << 20), u32::MAX);
                    }
                    creator[t as usize] = s;
                    continue;
                }
                if s == t {
                    selfe += 1;
                    continue;
                }
                if t < initial {
                    helper += 1;
                    continue;
                }
                if creator.get(t as usize).is_some_and(|&c| c == s) {
                    local += 1;
                    continue;
                }
                let age = m - t as u64;
                let mut i = 0;
                while age >= BOUNDS[i + 1] {
                    i += 1;
                }
                hist[i] += 1;
                seg_hist[i] += 1;
            }
            carry.drain(..whole);
        }
        let tot: u64 = seg_hist.iter().sum();
        let mut j = Json::new().s("kind", "lag-segment").s("label", &label).s("segment", &p.display().to_string()).u("first", first).u("count", count).u("hit_edges", tot);
        let mut cum = 0;
        for i in 0..11 {
            cum += seg_hist[i];
            j = j.f(&format!("stale_share_lag_lt_{}", BOUNDS[i + 1].min(1 << 40)), cum as f64 / tot.max(1) as f64);
        }
        out.line(j.done());
    }
    let tot: u64 = hist.iter().sum();
    let mut j = Json::new()
        .s("kind", "lag")
        .s("label", &label)
        .u("edges", edges)
        .u("creation_edges", creation)
        .u("self_edges", selfe)
        .u("local_edges", local)
        .u("helper_edges", helper)
        .u("layer_hit_edges", tot);
    let mut cum = 0;
    for i in 0..11 {
        cum += hist[i];
        j = j.u(&format!("age_lt_{}", BOUNDS[i + 1].min(1 << 40)), hist[i]).f(&format!("stale_share_lag_lt_{}", BOUNDS[i + 1].min(1 << 40)), cum as f64 / tot.max(1) as f64);
    }
    out.line(j.done());
}
