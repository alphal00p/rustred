//! `static` mode: sampled requests against the gen-N live index.
use crate::Args;
use crate::ckpt::{FLAG_INSPECTED, T};
use crate::l0::{self, L0};
use crate::soa::{self, Order, SQuery, Soa};
use crate::util::{Json, LoadMon, Rng, fit, kept, quantiles, thread_cpu_ns};
use crate::world::World;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

#[derive(Default, Clone, Copy, Debug)]
pub struct Rec {
    pub groups: u64,
    pub eligible: u64,
    pub blocks: u64,
    pub blocks_passed: u64,
    pub tested: u64,
    pub word_pass: u64,
    pub lane_pass: u64,
    pub exact_calls: u64,
    pub found: u64,
    pub maint_bound: u64,
    pub retired: u64,
}
impl Rec {
    pub fn add(&mut self, o: &Rec) {
        self.groups += o.groups;
        self.eligible += o.eligible;
        self.blocks += o.blocks;
        self.blocks_passed += o.blocks_passed;
        self.tested += o.tested;
        self.word_pass += o.word_pass;
        self.lane_pass += o.lane_pass;
        self.exact_calls += o.exact_calls;
        self.found += o.found;
        self.maint_bound += o.maint_bound;
        self.retired += o.retired;
    }
    fn from_l0(w: &l0::Work) -> Rec {
        Rec {
            groups: w.groups,
            eligible: w.groups_eligible,
            blocks: w.blocks,
            blocks_passed: w.blocks_passed,
            tested: w.tested,
            word_pass: w.tested - w.bit_rejected,
            lane_pass: w.exact_calls,
            exact_calls: w.exact_calls,
            found: w.found,
            maint_bound: w.maint_bound,
            retired: w.retired,
        }
    }
    fn from_soa(w: &soa::SWork) -> Rec {
        Rec {
            groups: w.groups,
            eligible: w.groups_eligible,
            blocks: w.blocks,
            blocks_passed: w.blocks_passed,
            tested: w.tested,
            word_pass: w.word_pass,
            lane_pass: w.lane_pass,
            exact_calls: w.exact_calls,
            found: w.found,
            maint_bound: 0,
            retired: w.retired,
        }
    }
}

pub struct Q {
    pub id: u32,
    pub bucket: usize,
    pub t: T,
    pub l0: l0::Query,
    pub s: SQuery,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Set {
    Miss,
    HitMin,
    HitFirst,
    Reverse,
    WordOnly,
}
impl Set {
    fn name(self) -> &'static str {
        match self {
            Set::Miss => "miss",
            Set::HitMin => "hit-minid",
            Set::HitFirst => "hit-firstfound",
            Set::Reverse => "reverse",
            Set::WordOnly => "word-only",
        }
    }
}

pub enum Layout<'a> {
    L0(&'a L0),
    Soa(&'a Soa),
}

/// One request; returns its work record.
#[inline(never)]
pub fn one(layout: &Layout, set: Set, q: &Q, t: &[T], exclude: bool) -> Rec {
    let ex = if exclude { q.id } else { u32::MAX };
    match layout {
        Layout::L0(l) => {
            let mut w = l0::Work::default();
            match set {
                Set::Miss | Set::HitMin => {
                    l.find(q.bucket, &q.l0, false, ex as usize, &mut w);
                }
                Set::HitFirst => {
                    l.find(q.bucket, &q.l0, true, ex as usize, &mut w);
                }
                Set::Reverse => {
                    l.reverse(q.bucket, &q.l0, ex as usize, &mut w);
                }
                Set::WordOnly => {
                    let pass = l.bits_only(q.bucket, &q.l0, ex as usize, &mut w);
                    w.bit_rejected = w.tested - pass;
                }
            }
            Rec::from_l0(&w)
        }
        Layout::Soa(s) => {
            let mut w = soa::SWork::default();
            match set {
                Set::Miss | Set::HitMin => {
                    s.find(q.bucket, &q.s, &q.t, t, false, ex, &mut w);
                }
                Set::HitFirst => {
                    s.find(q.bucket, &q.s, &q.t, t, true, ex, &mut w);
                }
                Set::Reverse => {
                    s.reverse(q.bucket, &q.s, &q.t, t, ex, &mut w);
                }
                Set::WordOnly => {
                    w.word_pass = s.word_only(q.bucket, &q.s, &mut w);
                }
            }
            Rec::from_soa(&w)
        }
    }
}

pub struct Out(std::fs::File);
impl Out {
    pub fn open(path: &str) -> Out {
        Out(std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap())
    }
    pub fn line(&mut self, s: String) {
        println!("{s}");
        writeln!(self.0, "{s}").unwrap();
    }
}

fn rec_json(j: Json, r: &Rec, n: u64, cpu_ns: u64) -> Json {
    let nf = n.max(1) as f64;
    j.u("n", n)
        .u("found", r.found)
        .f("found_frac", r.found as f64 / nf)
        .f("groups_per_q", r.groups as f64 / nf)
        .f("eligible_groups_per_q", r.eligible as f64 / nf)
        .f("blocks_per_q", r.blocks as f64 / nf)
        .f("blocks_passed_per_q", r.blocks_passed as f64 / nf)
        .f("tested_per_q", r.tested as f64 / nf)
        .f("word_pass_per_q", r.word_pass as f64 / nf)
        .f("lane_pass_per_q", r.lane_pass as f64 / nf)
        .f("exact_calls_per_q", r.exact_calls as f64 / nf)
        .f("maint_bound_per_q", r.maint_bound as f64 / nf)
        .f("retired_per_q", r.retired as f64 / nf)
        .u("cpu_ns", cpu_ns)
        .f("cpu_ns_per_q", cpu_ns as f64 / nf)
        .f("cpu_ns_per_tested", cpu_ns as f64 / r.tested.max(1) as f64)
}

/// Stats pass: one thread, every query once, per-query distributions.
pub fn stats_pass(layout: &Layout, set: Set, qs: &[Q], t: &[T], exclude: bool) -> (Rec, u64, Vec<u64>, Vec<(usize, u64, u64)>, (f64, f64)) {
    let mon = LoadMon::start();
    let mut tot = Rec::default();
    let mut per = Vec::with_capacity(qs.len());
    let mut per_bucket = Vec::with_capacity(qs.len());
    let c0 = thread_cpu_ns();
    for q in qs {
        let r = one(layout, set, q, t, exclude);
        per.push(r.tested);
        per_bucket.push((q.bucket, r.tested, r.found));
        tot.add(&r);
    }
    let cpu = thread_cpu_ns() - c0;
    let (f, s) = mon.stop();
    (tot, cpu, per, per_bucket, (f, s))
}

/// Throughput pass: `threads` threads cycle through the queries for
/// `seconds`; returns (records, CPU ns summed over threads, wall seconds).
pub fn throughput_pass(layout: &Layout, set: Set, qs: &[Q], t: &[T], exclude: bool, threads: usize, seconds: f64) -> (Rec, u64, u64, f64, (f64, f64)) {
    let mon = LoadMon::start();
    let cpu_total = AtomicU64::new(0);
    let queries = AtomicU64::new(0);
    let start = Instant::now();
    let recs: Vec<Rec> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..threads)
            .map(|k| {
                let (cpu_total, queries) = (&cpu_total, &queries);
                s.spawn(move || {
                    let mut tot = Rec::default();
                    let mut i = k * qs.len() / threads;
                    let c0 = thread_cpu_ns();
                    let t0 = Instant::now();
                    let mut n = 0u64;
                    while t0.elapsed().as_secs_f64() < seconds {
                        for _ in 0..16 {
                            let r = one(layout, set, &qs[i % qs.len()], t, exclude);
                            tot.add(&r);
                            i += 1;
                            n += 1;
                        }
                    }
                    cpu_total.fetch_add(thread_cpu_ns() - c0, Ordering::Relaxed);
                    queries.fetch_add(n, Ordering::Relaxed);
                    tot
                })
            })
            .collect();
        hs.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let wall = start.elapsed().as_secs_f64();
    let mut tot = Rec::default();
    for r in &recs {
        tot.add(r);
    }
    (tot, cpu_total.load(Ordering::Relaxed), queries.load(Ordering::Relaxed), wall, mon.stop())
}

pub fn make_q(w: &World, id: u32) -> Option<Q> {
    let t = w.t[id as usize];
    let bucket = w.bucket(&t)?;
    Some(Q {
        id,
        bucket,
        t,
        l0: l0::Query::of(&t),
        s: SQuery::of(&t),
    })
}

fn sample(pool: &[u32], k: usize, rng: &mut Rng) -> Vec<u32> {
    if pool.is_empty() {
        return Vec::new();
    }
    (0..k).map(|_| pool[rng.below(pool.len())]).collect()
}

pub fn run(args: &Args) {
    let dir = Path::new(args.req("ckpt"));
    let generation = args.req("gen");
    let samples = args.num("samples", 20000);
    let fracs = args.list("fracs", "1024,512,256");
    let threads = args.list("threads", "1");
    let seconds = args.num("seconds", 6) as f64;
    let pairs = args.num("pairs", 2_000_000);
    let label = args.get("label").unwrap_or("").to_string();
    let mut out = Out::open(args.get("out").unwrap_or("idxreplay-static.jsonl"));
    let load_threads = args.num("load-threads", 16);
    let w = World::load(dir, generation, load_threads);
    for r in &w.report {
        out.line(Json::new().s("kind", "world").s("label", &label).s("report", r).done());
    }
    let nseg = w.seg_first.len();
    let last_first = w.seg_first[nseg - 1];
    let prev_first = w.seg_first[nseg.saturating_sub(2)];
    let miss_pool: Vec<u32> = (last_first..w.n).filter(|&id| w.live[id]).map(|id| id as u32).collect();
    let deleg_pool: Vec<u32> = (prev_first..w.n)
        .filter(|&id| !w.live[id] && w.flags[id] & FLAG_INSPECTED == 0)
        .map(|id| id as u32)
        .collect();
    let insp_pool: Vec<u32> = (prev_first..w.n)
        .filter(|&id| !w.live[id] && w.flags[id] & FLAG_INSPECTED != 0)
        .map(|id| id as u32)
        .collect();
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let misses: Vec<Q> = sample(&miss_pool, samples, &mut rng).into_iter().filter_map(|id| make_q(&w, id)).collect();
    // Hit proxies: retired IDs, half never inspected (delegated), half inspected.
    let mut hit_ids = sample(&deleg_pool, samples / 2, &mut rng);
    hit_ids.extend(sample(&insp_pool, samples - samples / 2, &mut rng));
    let hits: Vec<Q> = hit_ids.into_iter().filter_map(|id| make_q(&w, id)).collect();
    out.line(
        Json::new()
            .s("kind", "pools")
            .s("label", &label)
            .u("miss_pool", miss_pool.len() as u64)
            .u("hit_pool_delegated", deleg_pool.len() as u64)
            .u("hit_pool_inspected_retired", insp_pool.len() as u64)
            .u("misses", misses.len() as u64)
            .u("hits", hits.len() as u64)
            .u("newest_segment_first", last_first as u64)
            .u("hit_pool_first", prev_first as u64)
            .done(),
    );

    // Differential check 1: lane test == exact containment on random
    // same-bucket pairs (both directions), for nonsaturating summaries.
    {
        let bucket_ids: Vec<Vec<u32>> = w
            .stored
            .iter()
            .map(|b| b.groups.iter().flat_map(|g| &g.blocks).flat_map(|bl| bl.ids.iter().copied()).collect())
            .collect();
        let (mut n, mut bad, mut pos) = (0u64, 0u64, 0u64);
        let mut r2 = Rng(12345);
        let all: Vec<&Q> = misses.iter().chain(hits.iter()).collect();
        for k in 0..pairs {
            let q = all[k % all.len()];
            let ids = &bucket_ids[q.bucket];
            if ids.is_empty() {
                continue;
            }
            // Mix random candidates with the query's own container chain.
            let c = &w.t[ids[r2.below(ids.len())] as usize];
            for (a, b) in [(c, &q.t), (&q.t, c)] {
                if soa::saturates(a) || soa::saturates(b) {
                    continue;
                }
                n += 1;
                let exact = a.contains(b);
                pos += exact as u64;
                if exact != soa::lane_contains(a, b) {
                    bad += 1;
                }
                if exact != l0::CompactSummary::of(a).contains(&l0::CompactSummary::of(b)) {
                    bad += 1;
                }
            }
        }
        out.line(
            Json::new()
                .s("kind", "pair-check")
                .s("label", &label)
                .u("pairs", n)
                .u("contained", pos)
                .u("mismatches", bad)
                .done(),
        );
    }

    let live_full = w.live_count(1024);
    let mut miss_by_bucket: Vec<(String, Vec<(usize, u64, u64)>)> = Vec::new();
    for &frac in &fracs {
        let frac = frac as u64;
        let t0 = Instant::now();
        let mut layouts: Vec<(String, Layout)> = Vec::new();
        let l0s = w.build_l0(frac, frac >= 1024);
        let l0r = if frac >= 1024 { Some(w.build_l0(frac, false)) } else { None };
        let keep = move |id: u32| kept(id, frac);
        let sid = Soa::build(&w.stored, &w.t, &w.words, &keep, Order::Id);
        let spat = Soa::build(&w.stored, &w.t, &w.words, &keep, Order::PatternLex);
        let live = w.live_count(frac);
        eprintln!("frac {frac}/1024: live {live}; layouts built in {:.1}s; soa blocks {} entries {}", t0.elapsed().as_secs_f64(), spat.blocks, spat.entries);
        // Differential check 2: SIMD vs scalar kernels on every block of the
        // buckets of the first 200 queries.
        let mut kbad = 0;
        for q in misses.iter().take(100).chain(hits.iter().take(100)) {
            kbad += sid.check_kernels(q.bucket, &q.s) + spat.check_kernels(q.bucket, &q.s);
        }
        out.line(Json::new().s("kind", "kernel-check").s("label", &label).u("frac", frac).u("mismatched_blocks", kbad as u64).done());
        layouts.push((if frac >= 1024 { "l0-stored" } else { "l0-rebuilt" }.into(), Layout::L0(&l0s)));
        if let Some(l) = &l0r {
            layouts.push(("l0-rebuilt".into(), Layout::L0(l)));
        }
        layouts.push(("soa-id".into(), Layout::Soa(&sid)));
        layouts.push(("soa-pattern".into(), Layout::Soa(&spat)));
        // Stats pass (one thread, every request once) unless --skip-stats 1.
        for (name, layout) in layouts.iter().filter(|_| args.get("skip-stats").is_none()) {
            for (set, qs, excl) in [
                (Set::Miss, &misses, true),
                (Set::HitMin, &hits, false),
                (Set::HitFirst, &hits, false),
                (Set::Reverse, &misses, true),
                (Set::WordOnly, &misses, true),
            ] {
                let (rec, cpu, per, per_bucket, (foreign, sib)) = stats_pass(layout, set, qs, &w.t, excl);
                let (p50, p90, p99, max) = quantiles(per);
                let j = Json::new()
                    .s("kind", "stats")
                    .s("label", &label)
                    .s("gen", generation)
                    .u("frac", frac)
                    .u("live", live as u64)
                    .f("live_frac", live as f64 / live_full as f64)
                    .s("layout", name)
                    .s("set", set.name())
                    .u("threads", 1);
                let j = rec_json(j, &rec, qs.len() as u64, cpu).u("tested_p50", p50).u("tested_p90", p90).u("tested_p99", p99).u("tested_max", max).f("foreign_busy_own_cpus", foreign).f("busy_smt_siblings", sib);
                out.line(j.done());
                if frac >= 1024 && set == Set::Miss {
                    miss_by_bucket.push((name.clone(), per_bucket));
                }
            }
        }
        for &th in &threads {
            for (name, layout) in &layouts {
                if name == "l0-rebuilt" && frac >= 1024 {
                    continue;
                }
                for (set, qs, excl) in [
                    (Set::Miss, &misses, true),
                    (Set::HitFirst, &hits, false),
                    (Set::HitMin, &hits, false),
                    (Set::Reverse, &misses, true),
                    (Set::WordOnly, &misses, true),
                ] {
                    let (rec, cpu, nq, wall, (foreign, sib)) = throughput_pass(layout, set, qs, &w.t, excl, th, seconds);
                    let j = Json::new()
                        .s("kind", "throughput")
                        .s("label", &label)
                        .s("gen", generation)
                        .u("frac", frac)
                        .u("live", live as u64)
                        .s("layout", name)
                        .s("set", set.name())
                        .u("threads", th as u64)
                        .f("wall_s", wall)
                        .f("queries_per_s", nq as f64 / wall)
                        .f("tested_per_s", rec.tested as f64 / wall)
                        .f("foreign_busy_own_cpus", foreign)
                        .f("busy_smt_siblings", sib);
                    out.line(rec_json(j, &rec, nq, cpu).done());
                }
            }
        }
    }
    // Per-bucket exponent of candidates per miss against bucket live size.
    let bl = w.bucket_live(1024);
    for (name, per) in &miss_by_bucket {
        let mut agg: std::collections::HashMap<usize, (u64, u64)> = Default::default();
        for &(b, tested, _) in per {
            let e = agg.entry(b).or_default();
            e.0 += tested;
            e.1 += 1;
        }
        let (mut xs, mut ys, mut ws) = (vec![], vec![], vec![]);
        for (&b, &(tested, n)) in &agg {
            if bl[b] > 0 && tested > 0 {
                xs.push((bl[b] as f64).ln());
                ys.push((tested as f64 / n as f64).ln());
                ws.push(n as f64);
            }
        }
        let (slope, icpt, r2) = fit(&xs, &ys, &ws);
        out.line(
            Json::new()
                .s("kind", "bucket-exponent")
                .s("label", &label)
                .s("layout", name)
                .s("set", "miss")
                .u("buckets", xs.len() as u64)
                .f("slope", slope)
                .f("intercept", icpt)
                .f("r2", r2)
                .done(),
        );
    }
}
