//! `streams` mode: real successor streams of a traced run resumed from the
//! checkpoint, replayed through the §3.2 pipeline, then the layer-reaching
//! requests are replayed against the checkpoint's live index on today's
//! layout and the SoA layouts (the snapshot = the checkpoint generation).
use crate::Args;
use crate::ckpt::{Raw, T};
use crate::dynamic::run_pipeline;
use crate::l0;
use crate::pipeline::{self, Ctx, LAYER_HIT, MISS};
use crate::soa::{Order, SQuery, Soa};
use crate::stat::{Layout, Out, Q, Rec, Set, one};
use crate::trace;
use crate::util::{Json, Rng, quantiles, thread_cpu_ns};
use crate::world::{World, digest};
use std::collections::HashMap;
use std::path::Path;

pub fn run(args: &Args) {
    let dir = Path::new(args.req("ckpt"));
    let generation = args.req("gen");
    let tdir = Path::new(args.req("trace"));
    let label = args.get("label").unwrap_or("").to_string();
    let max_layer = args.num("layer-sample", 200_000);
    let initial = args.num("initial", 67) as u32;
    let mut out = Out::open(args.get("out").unwrap_or("idxreplay-streams.jsonl"));
    let w = World::load(dir, generation, args.num("load-threads", 16));
    let coord = trace::files(tdir, "coord-");
    let mut recs = Vec::new();
    for c in &coord {
        recs.extend(trace::read_coord(c));
    }
    let jobs: Vec<trace::Job> = trace::files(tdir, "jobs-")
        .iter()
        .enumerate()
        .flat_map(|(k, p)| trace::read_jobs(p, k as u32))
        .collect();
    // Post-snapshot admissions of the traced run.
    let mut new_t: HashMap<u32, T> = HashMap::new();
    let mut new_exact: HashMap<Raw, u32> = HashMap::new();
    let mut kinds = [0u64; 5];
    let mut fwd_engine = [0u64; 5];
    for r in &recs {
        kinds[r.kind.min(4) as usize] += 1;
        fwd_engine[r.kind.min(4) as usize] += r.forward as u64;
        if r.kind == trace::NEW {
            new_t.insert(r.target, T::project(&r.raw));
            new_exact.insert(r.raw, r.target);
        }
    }
    let n7 = w.n as u32;
    out.line(
        Json::new()
            .s("kind", "streams-input")
            .s("label", &label)
            .u("admissions", recs.len() as u64)
            .u("exact", kinds[0])
            .u("orthant", kinds[1])
            .u("contained", kinds[2])
            .u("new", kinds[3])
            .u("refused", kinds[4])
            .u("engine_forward_checks_contained", fwd_engine[2])
            .u("engine_forward_checks_new", fwd_engine[3])
            .u("engine_maintenance", recs.iter().map(|r| r.maintenance).sum())
            .u("jobs_records", jobs.len() as u64)
            .u("snapshot_ids", n7 as u64)
            .u("post_snapshot_new_ids", new_t.len() as u64)
            .done(),
    );
    let t_of = |id: u32| if id < n7 { Some(w.t[id as usize]) } else { new_t.get(&id).copied() };
    let exact_probe = |r: &Raw| {
        w.exact.get(&digest(r)).copied().filter(|&id| {
            // Digest hit: confirm on the tight summary and bucket (a genuine
            // collision would only move a request between cheap tiers).
            let t = &w.t[id as usize];
            t.phase == r.phase && t.owner == r.owner && *t == T::project(r)
        }).or_else(|| new_exact.get(r).copied())
    };
    let mut helpers: HashMap<(u8, u16), Vec<u32>> = HashMap::new();
    for id in 0..initial.min(n7) {
        let t = &w.t[id as usize];
        helpers.entry((t.phase, t.owner)).or_default().push(id);
    }
    for b in &w.stored {
        if let Some(o) = b.orthant {
            let e = helpers.entry((b.phase, b.owner)).or_default();
            if !e.contains(&o) {
                e.push(o);
            }
        }
    }
    let src = pipeline::by_source(&recs);
    let wm = pipeline::watermarks(&recs, n7);
    let classified = run_pipeline(&mut out, &label, &jobs, &recs, &src, &wm, &Ctx { t_of: &t_of, exact: &exact_probe, helpers: &helpers });

    // Engine cost of today's path per request class (commit-time forward
    // checks, all requests reach the store in v2).
    // Layer-reaching requests under the pipeline (k = 64).
    let mut layer: Vec<(Q, bool, bool)> = Vec::new(); // (query, engine hit, stale)
    let (mut n_hit, mut n_miss, mut n_stale) = (0u64, 0u64, 0u64);
    for (_, v) in &classified {
        for c in v {
            if c.tier != LAYER_HIT && c.tier != MISS {
                continue;
            }
            let Some(t) = c.t else { continue };
            let Some(bucket) = w.bucket(&t) else { continue };
            let hit = c.tier == LAYER_HIT;
            let stale = hit && c.outcome.is_some_and(|o| o.target >= n7);
            n_hit += hit as u64;
            n_miss += !hit as u64;
            n_stale += stale as u64;
            layer.push((Q { id: u32::MAX, bucket, t, l0: l0::Query::of(&t), s: SQuery::of(&t) }, hit, stale));
        }
    }
    out.line(
        Json::new()
            .s("kind", "streams-layer-requests")
            .s("label", &label)
            .u("layer_hits", n_hit)
            .u("misses", n_miss)
            .u("stale_hits_post_snapshot_container", n_stale)
            .f("stale_share_of_layer_hits", n_stale as f64 / n_hit.max(1) as f64)
            .done(),
    );
    let mut rng = Rng(0xABCDEF);
    if layer.len() > max_layer {
        for i in 0..max_layer {
            let j = i + rng.below(layer.len() - i);
            layer.swap(i, j);
        }
        layer.truncate(max_layer);
    }
    let l0s = w.build_l0(1024, true);
    let sid = Soa::build(&w.stored, &w.t, &w.words, &|_| true, Order::Id);
    let spat = Soa::build(&w.stored, &w.t, &w.words, &|_| true, Order::PatternLex);
    let layouts: Vec<(&str, Layout)> = vec![("l0-stored", Layout::L0(&l0s)), ("soa-id", Layout::Soa(&sid)), ("soa-pattern", Layout::Soa(&spat))];
    for (cls, want_hit) in [("engine-hit", true), ("engine-miss", false)] {
        let qs: Vec<&Q> = layer.iter().filter(|x| x.1 == want_hit && !x.2).map(|x| &x.0).collect();
        for (name, lay) in &layouts {
            for set in [Set::HitMin, Set::HitFirst, Set::Reverse] {
                if !want_hit && set == Set::HitFirst {
                    continue;
                }
                if want_hit && set == Set::Reverse {
                    continue;
                }
                let mut tot = Rec::default();
                let mut per = Vec::with_capacity(qs.len());
                let c0 = thread_cpu_ns();
                for q in &qs {
                    let r = one(lay, set, q, &w.t, false);
                    per.push(r.tested);
                    tot.add(&r);
                }
                let cpu = thread_cpu_ns() - c0;
                let n = qs.len().max(1) as f64;
                let (p50, p90, p99, max) = quantiles(per);
                out.line(
                    Json::new()
                        .s("kind", "streams-layer-cost")
                        .s("label", &label)
                        .s("class", cls)
                        .s("layout", name)
                        .s("set", if set == Set::HitMin { if want_hit { "hit-minid" } else { "miss" } } else if set == Set::HitFirst { "hit-firstfound" } else { "reverse" })
                        .u("n", qs.len() as u64)
                        .f("found_frac", tot.found as f64 / n)
                        .f("tested_per_q", tot.tested as f64 / n)
                        .f("exact_calls_per_q", tot.exact_calls as f64 / n)
                        .f("blocks_passed_per_q", tot.blocks_passed as f64 / n)
                        .u("tested_p50", p50)
                        .u("tested_p90", p90)
                        .u("tested_p99", p99)
                        .u("tested_max", max)
                        .f("cpu_ns_per_q", cpu as f64 / n)
                        .f("cpu_ns_per_tested", cpu as f64 / tot.tested.max(1) as f64)
                        .done(),
                );
            }
        }
    }
}
