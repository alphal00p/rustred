//! The loaded checkpoint: per-ID tight summaries, filter words, raw-image
//! digests (exact tier), node flags, the stored candidate index and buckets.
use crate::ckpt::{self, N, Raw, Sig, StoredBucket, T};
use crate::l0::{self, L0};
use crate::util::kept;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

pub struct World {
    pub dir: PathBuf,
    pub n: usize,
    pub t: Vec<T>,
    pub words: Vec<u64>,
    pub flags: Vec<u8>,
    pub live: Vec<bool>,
    pub stored: Vec<StoredBucket>,
    pub bucket_of: HashMap<(u8, u16), usize>,
    /// raw-image digest -> first ID (exact tier; collisions only merge tiers)
    pub exact: HashMap<u64, u32>,
    pub seg_first: Vec<usize>,
    pub report: Vec<String>,
}

pub fn digest(r: &Raw) -> u64 {
    let mut h: u64 = 0x9E37_79B9_7F4A_7C15 ^ (r.phase as u64) << 17 ^ r.owner as u64;
    let mut mix = |v: u64| {
        h ^= v.wrapping_add(0x9E37_79B9_7F4A_7C15).wrapping_add(h << 6).wrapping_add(h >> 2);
        h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
        h ^= h >> 31;
    };
    for i in 0..N {
        mix((r.lower[i] as u64) << 16 | r.upper[i] as u64);
    }
    mix(r.rank.map_or(u64::MAX, |v| v as u64));
    mix(r.amax.map_or(u64::MAX - 1, |v| v));
    mix(r.dmin.map_or(u64::MAX - 2, |v| v as u64));
    mix(r.dmax.map_or(u64::MAX - 3, |v| v as u64));
    h
}

fn par_map<A: Sync, B: Send>(xs: &[A], threads: usize, f: impl Fn(&A) -> B + Sync) -> Vec<B> {
    let chunk = xs.len().div_ceil(threads.max(1)).max(1);
    std::thread::scope(|s| {
        let hs: Vec<_> = xs.chunks(chunk).map(|c| s.spawn(|| c.iter().map(&f).collect::<Vec<B>>())).collect();
        hs.into_iter().flat_map(|h| h.join().unwrap()).collect()
    })
}

impl World {
    pub fn load(dir: &Path, generation: &str, threads: usize) -> World {
        let t0 = std::time::Instant::now();
        let segs = ckpt::load_domains(dir, None);
        let seg_first: Vec<usize> = segs.iter().map(|s| s.first).collect();
        let n: usize = segs.iter().map(|s| s.count).sum();
        eprintln!("decoded {n} domains in {:.1}s", t0.elapsed().as_secs_f64());
        let mut t = Vec::with_capacity(n);
        let mut exact: HashMap<u64, u32> = HashMap::with_capacity(n);
        let mut id = 0u32;
        for s in segs {
            let ts = par_map(&s.raws, threads, |r| (T::project(r), digest(r)));
            for (tt, d) in ts {
                t.push(tt);
                exact.entry(d).or_insert(id);
                id += 1;
            }
        }
        let words = par_map(&t, threads, |x| x.word());
        eprintln!("projected in {:.1}s", t0.elapsed().as_secs_f64());
        let flags = ckpt::load_nodes(&dir.join(format!("nodes-{generation}.bin")));
        assert_eq!(flags.len(), n, "nodes vs domains");
        let stored = ckpt::load_index(&dir.join(format!("index-{generation}.bin")));
        let mut live = vec![false; n];
        let mut bucket_of = HashMap::new();
        let mut report = Vec::new();
        let (mut sig_mismatch, mut env_violation, mut owner_mismatch, mut groups, mut blocks, mut nlive) =
            (0usize, 0usize, 0usize, 0usize, 0usize, 0usize);
        for (bi, b) in stored.iter().enumerate() {
            bucket_of.insert((b.phase, b.owner), bi);
            for g in &b.groups {
                groups += 1;
                for bl in &g.blocks {
                    blocks += 1;
                    for &id in &bl.ids {
                        let s = &t[id as usize];
                        live[id as usize] = true;
                        nlive += 1;
                        if Sig::of(s) != g.sig {
                            sig_mismatch += 1;
                        }
                        if s.owner != b.owner || s.phase != b.phase {
                            owner_mismatch += 1;
                        }
                        if bl.env.len() == N
                            && !s.empty
                            && !(0..N).all(|i| {
                                let e = &bl.env[i];
                                let up = (s.up[i] != ckpt::U16INF).then_some(s.up[i] as u64);
                                e.min_lower <= s.lo[i] as u64
                                    && s.lo[i] as u64 <= e.max_lower
                                    && e.max_upper.is_none_or(|a| up.is_some_and(|b| b <= a))
                                    && up.is_none_or(|u| e.min_upper.is_some_and(|m| m <= u))
                            })
                        {
                            env_violation += 1;
                        }
                    }
                }
            }
        }
        let sat = t.iter().filter(|x| crate::soa::saturates(x)).count();
        let empty = t.iter().filter(|x| x.empty).count();
        report.push(format!(
            "gen {generation}: domains {n} buckets {} groups {groups} blocks {blocks} live {nlive}; checks: signature mismatches {sig_mismatch}, owner/phase mismatches {owner_mismatch}, stored-envelope violations {env_violation}, projection clamps {}, u8-lane saturating summaries {sat}, empty summaries {empty}, distinct raw digests {}",
            stored.len(),
            ckpt::CLAMPS.load(Ordering::Relaxed),
            exact.len()
        ));
        eprintln!("{}", report.last().unwrap());
        eprintln!("world ready in {:.1}s", t0.elapsed().as_secs_f64());
        World {
            dir: dir.to_path_buf(),
            n,
            t,
            words,
            flags,
            live,
            stored,
            bucket_of,
            exact,
            seg_first,
            report,
        }
    }

    pub fn bucket(&self, t: &T) -> Option<usize> {
        self.bucket_of.get(&(t.phase, t.owner)).copied()
    }

    /// Today's layout. `stored=true` keeps the persisted blocks and
    /// envelopes (exact gen state, fill as persisted); otherwise candidates
    /// kept by `per1024` are re-chunked into full blocks with exact envelopes.
    pub fn build_l0(&self, per1024: u64, stored: bool) -> L0 {
        let mut slots = vec![l0::RELEASED; self.n];
        let mut entries = Vec::new();
        for id in 0..self.n {
            if self.live[id] && kept(id as u32, per1024) {
                slots[id] = entries.len() as u32;
                entries.push(l0::CompactSummary::of(&self.t[id]));
            }
        }
        let buckets = self
            .stored
            .iter()
            .map(|b| l0::Bucket {
                groups: b
                    .groups
                    .iter()
                    .filter_map(|g| {
                        let blocks: Vec<l0::Block> = if stored {
                            g.blocks
                                .iter()
                                .map(|bl| {
                                    let mut ids = [0usize; 32];
                                    for (k, &id) in bl.ids.iter().enumerate() {
                                        ids[k] = id as usize;
                                    }
                                    l0::Block {
                                        ids,
                                        len: bl.ids.len(),
                                        envelope: bl
                                            .env
                                            .iter()
                                            .map(|e| l0::AxisEnvelope {
                                                min_lower: e.min_lower,
                                                max_lower: e.max_lower,
                                                min_upper: e.min_upper,
                                                max_upper: e.max_upper,
                                            })
                                            .collect(),
                                    }
                                })
                                .collect()
                        } else {
                            let ids: Vec<usize> = g
                                .blocks
                                .iter()
                                .flat_map(|bl| bl.ids.iter())
                                .filter(|&&id| kept(id, per1024))
                                .map(|&id| id as usize)
                                .collect();
                            ids.chunks(32)
                                .map(|c| {
                                    let mut ids = [0usize; 32];
                                    ids[..c.len()].copy_from_slice(c);
                                    l0::Block {
                                        ids,
                                        len: c.len(),
                                        envelope: l0::envelope_of(c, &self.t),
                                    }
                                })
                                .collect()
                        };
                        let live = blocks.iter().map(|b| b.len).sum();
                        (live > 0 || stored).then_some(l0::Group {
                            signature: l0::Signature::of(&g.sig),
                            blocks,
                            live,
                        })
                    })
                    .collect(),
            })
            .collect();
        L0 {
            buckets,
            bits: self.words.clone(),
            slots,
            entries,
        }
    }

    pub fn live_count(&self, per1024: u64) -> usize {
        (0..self.n).filter(|&id| self.live[id] && kept(id as u32, per1024)).count()
    }

    pub fn bucket_live(&self, per1024: u64) -> Vec<usize> {
        self.stored
            .iter()
            .map(|b| {
                b.groups
                    .iter()
                    .flat_map(|g| &g.blocks)
                    .flat_map(|bl| &bl.ids)
                    .filter(|&&id| kept(id, per1024))
                    .count()
            })
            .collect()
    }
}
