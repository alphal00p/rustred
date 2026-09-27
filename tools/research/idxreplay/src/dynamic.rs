//! `dynamic` mode: rebuild today's candidate index from scratch in the
//! coordinator's commit order of a traced run (`Queue::admit_with_lookup`,
//! `AggregateIndex::{find_controlled, maintenance_len, retire, insert}`) and
//! check every admission's outcome and counter deltas against the engine.
//! On the same evolving state it also measures first-found forward cost, the
//! SoA-in-ID-order forward cost, and classifies every request of every joined
//! inspection through the §3.2 pipeline.
use crate::Args;
use crate::ckpt::{N, Raw, Sig, T, U16INF};
use crate::l0::{self, AxisEnvelope, CompactSummary, Coords, Signature};
use crate::pipeline::{self, Ctx, Outcome, Tally, TIERS};
use crate::stat::Out;
use crate::trace::{self, Admission};
use crate::util::{Json, fit, quantiles, thread_cpu_ns};
use std::collections::HashMap;
use std::path::Path;

struct DBlock {
    ids: Vec<usize>,
    env: Vec<AxisEnvelope>,
}
struct DGroup {
    sig: Signature,
    blocks: Vec<DBlock>,
    live: usize,
}
#[derive(Default)]
struct DBucket {
    groups: Vec<DGroup>,
    positions: HashMap<SigKey, usize>,
    orthant: Option<usize>,
    live: usize,
}
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SigKey(bool, u32, u32, i32);
fn key(s: &Sig) -> SigKey {
    SigKey(s.empty, s.pos, s.num, s.dif)
}

fn upper_contains(container: Option<u64>, candidate: Option<u64>) -> bool {
    container.is_none_or(|a| candidate.is_some_and(|b| a >= b))
}
fn may_contain(env: &[AxisEnvelope], q: Option<&Coords>) -> bool {
    q.is_none_or(|q| {
        env.iter()
            .zip(q.lower.iter().zip(&q.upper))
            .all(|(a, (&l, &u))| a.min_lower <= l && upper_contains(a.max_upper, u))
    })
}
fn may_be_contained(env: &[AxisEnvelope], q: Option<&Coords>) -> bool {
    q.is_none_or(|q| {
        env.iter()
            .zip(q.lower.iter().zip(&q.upper))
            .all(|(a, (&l, &u))| l <= a.max_lower && upper_contains(u, a.min_upper))
    })
}
fn env_insert(env: &mut Vec<AxisEnvelope>, q: Option<&Coords>) {
    let Some(q) = q else { return };
    if env.is_empty() {
        *env = (0..N)
            .map(|i| AxisEnvelope {
                min_lower: q.lower[i],
                max_lower: q.lower[i],
                min_upper: q.upper[i],
                max_upper: q.upper[i],
            })
            .collect();
        return;
    }
    for (i, a) in env.iter_mut().enumerate() {
        let (l, u) = (q.lower[i], q.upper[i]);
        a.min_lower = a.min_lower.min(l);
        a.max_lower = a.max_lower.max(l);
        a.min_upper = match (a.min_upper, u) {
            (Some(x), Some(y)) => Some(x.min(y)),
            (x, y) => x.or(y),
        };
        a.max_upper = match (a.max_upper, u) {
            (Some(x), Some(y)) => Some(x.max(y)),
            _ => None,
        };
    }
}

fn rank_contains(c: Option<u32>, q: Option<u32>) -> bool {
    c.is_none_or(|r| q.is_some_and(|s| s <= r))
}
fn full_orthant(r: &Raw, arity: usize) -> bool {
    r.amax.is_none()
        && r.dmin.is_none()
        && r.dmax.is_none()
        && r.lower[..arity].iter().all(|&x| x == 0)
        && r.upper[..arity].iter().all(|&x| x == U16INF)
}

pub struct Dyn {
    pub arity: usize,
    pub raws: Vec<Raw>,
    pub t: Vec<T>,
    pub words: Vec<u64>,
    pub cs: Vec<CompactSummary>,
    pub live: Vec<bool>,
    exact: HashMap<Raw, u32>,
    buckets: HashMap<(u8, u16), DBucket>,
}

#[derive(Default, Clone, Copy)]
pub struct Step {
    pub kind: u8,
    pub target: usize,
    pub forward: u64,
    pub forward_first: u64,
    pub maintenance: u64,
    pub retired: u64,
    pub reverse_tested: u64,
    pub bucket_live: usize,
}

impl Dyn {
    pub fn new(arity: usize) -> Self {
        Dyn {
            arity,
            raws: Vec::new(),
            t: Vec::new(),
            words: Vec::new(),
            cs: Vec::new(),
            live: Vec::new(),
            exact: HashMap::new(),
            buckets: HashMap::new(),
        }
    }

    /// Forward scan over the current state (min-ID or first-found).
    fn find(&self, b: &DBucket, q: &l0::Query, first_found: bool) -> (Option<usize>, u64) {
        let coords = q.coords.as_ref();
        let mut best: Option<usize> = None;
        let mut tested = 0u64;
        'g: for g in &b.groups {
            if !g.sig.may_contain(q.signature) {
                continue;
            }
            for blk in &g.blocks {
                if blk.ids.is_empty() {
                    continue;
                }
                if best.is_some_and(|best| blk.ids[0] >= best) {
                    break;
                }
                if !may_contain(&blk.env, coords) {
                    continue;
                }
                for &id in &blk.ids {
                    if best.is_some_and(|best| id >= best) {
                        break;
                    }
                    tested += 1;
                    if q.word & !self.words[id] != 0 {
                        continue;
                    }
                    if self.cs[id].contains(&q.compact) {
                        best = Some(id);
                        if first_found {
                            break 'g;
                        }
                        break;
                    }
                }
            }
        }
        (best, tested)
    }

    /// `Queue::admit_with_lookup` on the unlimited lane.
    pub fn admit(&mut self, raw: Raw) -> Step {
        let mut st = Step::default();
        if let Some(&id) = self.exact.get(&raw) {
            st.kind = trace::EXACT;
            st.target = id as usize;
            return st;
        }
        let t = T::project(&raw);
        let q = l0::Query::of(&t);
        let bk = (raw.phase, raw.owner);
        if let Some(b) = self.buckets.get(&bk) {
            st.bucket_live = b.live;
            if let Some(o) = b.orthant
                && rank_contains(self.raws[o].rank, raw.rank)
            {
                st.kind = trace::ORTHANT;
                st.target = o;
                return st;
            }
            let (found, tested) = self.find(b, &q, false);
            st.forward = tested;
            let (_, tested_ff) = self.find(b, &q, true);
            st.forward_first = tested_ff;
            if let Some(id) = found {
                st.kind = trace::CONTAINED;
                st.target = id;
                return st;
            }
        }
        let id = self.raws.len();
        st.kind = trace::NEW;
        st.target = id;
        let sig = Sig::of(&t);
        let sigl = Signature::of(&sig);
        let coords = q.coords.as_ref();
        let b = self.buckets.entry(bk).or_default();
        // Insertion preparation (before retirement), as `prepare_with`.
        let (new_group, new_block) = match b.positions.get(&key(&sig)) {
            Some(&p) => (false, !b.groups[p].blocks.last().is_some_and(|x| x.ids.len() < 32)),
            None => (true, true),
        };
        // maintenance_len: live of groups the new signature may contain.
        st.maintenance = b.groups.iter().filter(|g| sigl.may_contain(g.sig)).map(|g| g.live as u64).sum();
        // retire
        let mut position = 0;
        let mut removed = 0usize;
        while position < b.groups.len() {
            let eligible = sigl.may_contain(b.groups[position].sig);
            let g = &mut b.groups[position];
            if eligible {
                for blk in g.blocks.iter_mut() {
                    if !may_be_contained(&blk.env, coords) {
                        continue;
                    }
                    let before = blk.ids.len();
                    let (words, cs, live) = (&self.words, &self.cs, &mut self.live);
                    blk.ids.retain(|&old| {
                        st.reverse_tested += 1;
                        let retire = words[old] & !q.word == 0 && q.compact.contains(&cs[old]);
                        if retire {
                            live[old] = false;
                        }
                        !retire
                    });
                    let r = before - blk.ids.len();
                    removed += r;
                    g.live -= r;
                }
                let pin_tail = g.sig == sigl && !new_block;
                let old_len = g.blocks.len();
                let mut k = 0;
                g.blocks.retain(|blk| {
                    k += 1;
                    !blk.ids.is_empty() || (pin_tail && k == old_len)
                });
            }
            if g.blocks.is_empty() && g.sig != sigl {
                let gone = b.groups.swap_remove(position);
                let gone_key = b.positions.iter().find(|&(_, &p)| p == position).map(|(k, _)| *k);
                if let Some(k) = gone_key {
                    b.positions.remove(&k);
                }
                let _ = gone;
                if position < b.groups.len() {
                    let moved = b.groups.len();
                    // The group moved from the old last position `moved` to `position`.
                    for p in b.positions.values_mut() {
                        if *p == moved {
                            *p = position;
                        }
                    }
                }
            } else {
                position += 1;
            }
        }
        st.retired = removed as u64;
        b.live -= removed;
        // insert
        if new_group {
            let mut blk = DBlock { ids: Vec::with_capacity(32), env: Vec::new() };
            env_insert(&mut blk.env, coords);
            blk.ids.push(id);
            b.positions.insert(key(&sig), b.groups.len());
            b.groups.push(DGroup { sig: sigl, blocks: vec![blk], live: 1 });
        } else {
            let p = b.positions[&key(&sig)];
            let g = &mut b.groups[p];
            if new_block {
                g.blocks.push(DBlock { ids: Vec::with_capacity(32), env: Vec::new() });
            }
            let blk = g.blocks.last_mut().unwrap();
            env_insert(&mut blk.env, coords);
            blk.ids.push(id);
            g.live += 1;
        }
        b.live += 1;
        if full_orthant(&raw, self.arity) && b.orthant.is_none_or(|old| rank_contains(raw.rank, self.raws[old].rank)) {
            b.orthant = Some(id);
        }
        self.exact.insert(raw, id as u32);
        self.raws.push(raw);
        self.words.push(q.word);
        self.cs.push(q.compact);
        self.t.push(t);
        self.live.push(true);
        st
    }
}

pub fn run(args: &Args) {
    let dir = Path::new(args.req("trace"));
    let label = args.get("label").unwrap_or("").to_string();
    let mut out = Out::open(args.get("out").unwrap_or("idxreplay-dynamic.jsonl"));
    let coord = trace::files(dir, "coord-");
    assert_eq!(coord.len(), 1, "one coordinator trace per directory");
    let (arity, recs) = trace::read_coord_arity(&coord[0]);
    let mut d = Dyn::new(arity);
    let mut steps: Vec<Step> = Vec::with_capacity(recs.len());
    let (mut kind_mm, mut fwd_mm, mut maint_mm, mut ret_mm) = (0u64, 0u64, 0u64, 0u64);
    let (mut fwd_engine, mut fwd_replay, mut fwd_first, mut maint_engine, mut maint_replay) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut first_mm = Vec::new();
    let c0 = thread_cpu_ns();
    for (i, r) in recs.iter().enumerate() {
        let st = d.admit(r.raw);
        if st.kind != r.kind || (st.target as u32) != r.target {
            kind_mm += 1;
            if first_mm.len() < 5 {
                first_mm.push(format!("#{i}: engine kind {} target {} vs replay kind {} target {}", r.kind, r.target, st.kind, st.target));
            }
        }
        fwd_mm += (st.forward != r.forward as u64) as u64;
        maint_mm += (st.maintenance != r.maintenance) as u64;
        ret_mm += (st.retired != r.retired as u64) as u64;
        fwd_engine += r.forward as u64;
        fwd_replay += st.forward;
        fwd_first += st.forward_first;
        maint_engine += r.maintenance;
        maint_replay += st.maintenance;
        steps.push(st);
    }
    let cpu = thread_cpu_ns() - c0;
    let mut kinds = [0u64; 5];
    for r in &recs {
        kinds[r.kind.min(4) as usize] += 1;
    }
    out.line(
        Json::new()
            .s("kind", "dynamic-validation")
            .s("label", &label)
            .s("trace", &coord[0].display().to_string())
            .u("admissions", recs.len() as u64)
            .u("exact", kinds[0])
            .u("orthant", kinds[1])
            .u("contained", kinds[2])
            .u("new", kinds[3])
            .u("refused", kinds[4])
            .u("outcome_mismatches", kind_mm)
            .u("forward_mismatches", fwd_mm)
            .u("maintenance_mismatches", maint_mm)
            .u("retired_mismatches", ret_mm)
            .u("forward_checks_engine", fwd_engine)
            .u("forward_checks_replay", fwd_replay)
            .u("forward_checks_first_found", fwd_first)
            .u("maintenance_engine", maint_engine)
            .u("maintenance_replay", maint_replay)
            .u("reverse_tested_replay", steps.iter().map(|s| s.reverse_tested).sum())
            .u("replay_cpu_ns", cpu)
            .s("first_mismatches", &first_mm.join(" | "))
            .done(),
    );
    // Cost per request by outcome, and the dynamic exponent of candidates per
    // miss against bucket live size (as the walk grows).
    for (name, sel) in [("hit", trace::CONTAINED), ("miss", trace::NEW)] {
        let v: Vec<&Step> = steps.iter().filter(|s| s.kind == sel && s.bucket_live > 0).collect();
        let n = v.len().max(1) as f64;
        let (p50, p90, p99, max) = quantiles(v.iter().map(|s| s.forward).collect());
        let (xs, ys, ws): (Vec<f64>, Vec<f64>, Vec<f64>) = {
            // Bin by log2 bucket live size to weight evenly across sizes.
            let mut bins: HashMap<u32, (f64, f64)> = HashMap::new();
            for s in &v {
                let b = (usize::BITS - s.bucket_live.leading_zeros()) as u32;
                let e = bins.entry(b).or_default();
                e.0 += s.forward as f64;
                e.1 += 1.0;
            }
            let mut xs = vec![];
            let mut ys = vec![];
            let mut ws = vec![];
            for (b, (sum, cnt)) in bins {
                if sum > 0.0 && cnt >= 20.0 {
                    xs.push((1u64 << b.saturating_sub(1)) as f64 * 1.5f64.ln().exp());
                    ys.push(sum / cnt);
                    ws.push(cnt);
                }
            }
            (xs.iter().map(|x| x.ln()).collect(), ys.iter().map(|y| y.ln()).collect(), ws)
        };
        let (slope, _, r2) = fit(&xs, &ys, &ws);
        out.line(
            Json::new()
                .s("kind", "dynamic-cost")
                .s("label", &label)
                .s("outcome", name)
                .u("n", v.len() as u64)
                .f("forward_minid_per_q", v.iter().map(|s| s.forward).sum::<u64>() as f64 / n)
                .f("forward_firstfound_per_q", v.iter().map(|s| s.forward_first).sum::<u64>() as f64 / n)
                .f("reverse_tested_per_q", v.iter().map(|s| s.reverse_tested).sum::<u64>() as f64 / n)
                .u("forward_p50", p50)
                .u("forward_p90", p90)
                .u("forward_p99", p99)
                .u("forward_max", max)
                .f("exponent_vs_bucket_live", slope)
                .f("exponent_r2", r2)
                .u("exponent_bins", xs.len() as u64)
                .done(),
        );
    }
    // Pipeline on the joined job streams.
    let jobs: Vec<trace::Job> = trace::files(dir, "jobs-")
        .iter()
        .enumerate()
        .flat_map(|(k, p)| trace::read_jobs(p, k as u32))
        .collect();
    let src = pipeline::by_source(&recs);
    let initial: Vec<u32> = recs.iter().filter(|r| r.source == u32::MAX && r.kind == trace::NEW).map(|r| r.target).collect();
    let mut helpers: HashMap<(u8, u16), Vec<u32>> = HashMap::new();
    for &h in &initial {
        let r = &d.raws[h as usize];
        helpers.entry((r.phase, r.owner)).or_default().push(h);
    }
    let t_of = |id: u32| d.t.get(id as usize).copied();
    let exact_probe = |r: &Raw| d.exact.get(r).copied();
    let wm = pipeline::watermarks(&recs, 0);
    let classified = run_pipeline(&mut out, &label, &jobs, &recs, &src, &wm, &Ctx { t_of: &t_of, exact: &exact_probe, helpers: &helpers });
    // Candidate work today (every committed request reaches exact, orthant
    // and the min-ID scan) vs after the pipeline (only layer-reaching
    // requests scan, first-found), on the same evolving index (k = 64).
    let (mut today_fwd, mut pipe_fwd_min, mut pipe_fwd_ff, mut pipe_rev, mut layer_req, mut all_req) = (0u64, 0u64, 0u64, 0u64, 0u64, 0u64);
    for s in &steps {
        today_fwd += s.forward;
    }
    for (_, v) in &classified {
        for c in v {
            all_req += c.count;
            if c.tier == pipeline::LAYER_HIT || c.tier == pipeline::MISS {
                layer_req += c.count;
                if let Some(o) = c.outcome {
                    let s = &steps[o.rec as usize];
                    pipe_fwd_min += s.forward;
                    pipe_fwd_ff += s.forward_first;
                    pipe_rev += s.reverse_tested;
                }
            }
        }
    }
    out.line(
        Json::new()
            .s("kind", "dynamic-pipeline-work")
            .s("label", &label)
            .u("joined_requests", all_req)
            .u("layer_requests", layer_req)
            .u("forward_candidates_today_all_commits", today_fwd)
            .u("forward_candidates_pipeline_minid", pipe_fwd_min)
            .u("forward_candidates_pipeline_firstfound", pipe_fwd_ff)
            .u("reverse_candidates_pipeline", pipe_rev)
            .u("reverse_candidates_today", steps.iter().map(|s| s.reverse_tested).sum())
            .done(),
    );
}

/// Pipeline tallies for k = 1/4/16/64, overall and per phase; optional layer
/// cost callback for layer-reaching requests is handled by the caller.
pub fn run_pipeline(
    out: &mut Out,
    label: &str,
    jobs: &[trace::Job],
    recs: &[Admission],
    src: &HashMap<u32, Vec<usize>>,
    wm: &[u32],
    ctx: &Ctx,
) -> Vec<(u32, Vec<pipeline::Classified>)> {
    // Keep one complete record per job id (the last complete one).
    let mut best: HashMap<u32, &trace::Job> = HashMap::new();
    let (mut stopped, mut errored) = (0u64, 0u64);
    for j in jobs {
        if j.stopped {
            stopped += 1;
            continue;
        }
        if j.error {
            errored += 1;
            continue;
        }
        best.insert(j.id, j);
    }
    let mut kept_classified = Vec::new();
    let mut lag_hist = [0u64; LAG_BOUNDS.len() + 1];
    for k in [1usize, 4, 16, 64] {
        let mut per_phase = [Tally::default(), Tally::default()];
        let mut unjoined_jobs = 0u64;
        let mut unjoined_events = 0u64;
        let mut native_seconds = [0f64; 2];
        for (&id, j) in &best {
            let outcomes = pipeline::join(j, recs, wm, src.get(&id));
            if outcomes.is_none() {
                unjoined_jobs += 1;
                unjoined_events += j.events.len() as u64;
                continue;
            }
            let mut v = Vec::new();
            let ph = j.parent.phase as usize;
            native_seconds[ph.min(1)] += j.seconds;
            pipeline::classify(j, outcomes.as_deref(), ctx, k, &mut per_phase[ph.min(1)], &mut v);
            if k == 16 {
                for c in &v {
                    if c.tier == pipeline::LAYER_HIT
                        && let Some(o) = c.outcome
                    {
                        let age = o.watermark.saturating_sub(o.target) as u64;
                        let b = LAG_BOUNDS.iter().position(|&x| age < x).unwrap_or(LAG_BOUNDS.len());
                        lag_hist[b] += c.count;
                    }
                }
            }
            if k == 64 {
                kept_classified.push((id, v));
            }
        }
        let mut all = per_phase[0].clone();
        all.add(&per_phase[1]);
        let secs = [native_seconds[0] + native_seconds[1], native_seconds[0], native_seconds[1]];
        for (si, (name, t)) in [("all", &all), ("apply-jobs", &per_phase[0]), ("route-jobs", &per_phase[1])].into_iter().enumerate() {
            let tot = t.total().max(1) as f64;
            let mut j = Json::new()
                .f("native_seconds", secs[si])
                .f("native_ms_per_job", 1000.0 * secs[si] / t.jobs.max(1) as f64)
                .f("requests_per_job", t.total() as f64 / t.jobs.max(1) as f64)
                .s("kind", "pipeline")
                .s("label", label)
                .u("mru_k", k as u64)
                .s("scope", name)
                .u("jobs", t.jobs)
                .u("jobs_unjoined_skipped", unjoined_jobs)
                .u("unjoined_event_records", unjoined_events)
                .u("jobs_stopped_skipped", stopped)
                .u("jobs_error_skipped", errored)
                .u("requests", t.total())
                .f("cheap_share", t.cheap() as f64 / tot)
                .f("local_tests_per_req", t.local_tests as f64 / tot)
                .f("mru_tests_per_req", t.mru_tests as f64 / tot)
                .f("helper_tests_per_req", t.helper_tests as f64 / tot);
            for (i, n) in TIERS.iter().enumerate() {
                j = j.f(&format!("share_{n}"), t.requests[i] as f64 / tot);
            }
            out.line(j.done());
        }
    }
    let tot: u64 = lag_hist.iter().sum();
    let mut j = Json::new().s("kind", "stale-lag-requests").s("label", label).u("layer_hit_requests_k16", tot);
    let mut cum = 0;
    for (i, b) in LAG_BOUNDS.iter().enumerate() {
        cum += lag_hist[i];
        j = j.f(&format!("stale_share_lag_lt_{b}"), cum as f64 / tot.max(1) as f64);
    }
    out.line(j.done());
    kept_classified
}

const LAG_BOUNDS: [u64; 8] = [64, 1024, 4096, 16384, 65536, 262_144, 1_048_576, 4_194_304];

pub fn _unused(_: Outcome) {}
