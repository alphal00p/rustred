//! Gen-7 kernel harness (W1.1 gates b and c), ignored by default.
//!
//! Restores only the queue sections of a CP5 state checkpoint (a read-only
//! clone of the v2 campaign's generation 7) into the production queue (the
//! struct-of-arrays kernel) and into the pre-kernel layout (`legacy`), then:
//!
//! 1. checks every admitted ID's derived kernel word and lanes against the
//!    native summary;
//! 2. differential: real admission requests of the traced gen-7 resume (plus
//!    synthetic wide, empty, infinite and saturated variants) are looked up
//!    forward and reverse in both layouts; every logical candidate the kernel
//!    rejects or tests is compared with the historical per-ID predicate, the
//!    lookups must agree on the result and the charged count, and a sweep
//!    evaluates the kernel on every live slot of the request's bucket against
//!    the historical predicate (necessity everywhere, equality where both
//!    sides are unsaturated);
//! 3. timing: single-thread forward and reverse lookups, interleaved
//!    old/new per chunk, CPU from `/proc/thread-self/schedstat`.
//!
//! Environment: `RUSTRED_KERNEL_CKPT` (checkpoint directory),
//! `RUSTRED_KERNEL_OUT` (JSON receipt), optional `RUSTRED_KERNEL_TRACE`
//! (`coord-*.bin` of the admission trace), `RUSTRED_KERNEL_STRIDE` (sample
//! every k-th traced index request, default 400), `RUSTRED_KERNEL_LIMIT`
//! (requests, default 60000), `RUSTRED_KERNEL_SWEEP` (sweep every k-th
//! request, default 20), `RUSTRED_KERNEL_THREADS` (differential threads,
//! default 16), `RUSTRED_KERNEL_TIMING` (timed requests, default 20000).
use super::legacy::{self, Legacy};
use super::*;
use crate::application::routed_campaign::walking::checkpoint::restore::{
    Phases, read_queue_sections,
};
use serde_json::{Value, json};
use std::io::Read;
use std::time::Instant;

const N: usize = 15;

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// (on-CPU ns, run-queue wait ns) of this thread.
fn schedstat() -> (u64, u64) {
    let text = std::fs::read_to_string("/proc/thread-self/schedstat").unwrap_or_default();
    let mut fields = text
        .split_whitespace()
        .map(|v| v.parse::<u64>().unwrap_or(0));
    (fields.next().unwrap_or(0), fields.next().unwrap_or(0))
}

fn current_cpu() -> Option<usize> {
    let text = std::fs::read_to_string("/proc/thread-self/stat").ok()?;
    let after = &text[text.rfind(')')? + 2..];
    after.split_whitespace().nth(36)?.parse().ok()
}

/// (busy, total) jiffies of one CPU.
fn cpu_jiffies(cpu: usize) -> Option<(u64, u64)> {
    let text = std::fs::read_to_string("/proc/stat").ok()?;
    let prefix = format!("cpu{cpu} ");
    let line = text.lines().find(|line| line.starts_with(&prefix))?;
    let v: Vec<u64> = line
        .split_whitespace()
        .skip(1)
        .map(|x| x.parse().unwrap_or(0))
        .collect();
    let total: u64 = v.iter().take(8).sum();
    Some((total - v[3] - v[4], total))
}

struct Request {
    key: (Phase, [bool; N]),
    query: Query<N>,
    old: legacy::OldQuery<N>,
    kind: &'static str,
}

fn request(domain: Domain<N>, kind: &'static str) -> Option<Request> {
    let core = DomainPowerSummary::try_new(
        domain.owner,
        &domain.lower,
        &domain.upper,
        domain.rank,
        domain.powers,
    )
    .ok()?;
    let query = Query::new(core, domain.phase);
    Some(Request {
        key: (domain.phase, domain.owner),
        old: legacy::OldQuery::of(&query),
        query,
        kind,
    })
}

/// Synthetic variants of a real request, in its own bucket.
fn variants(domain: &Domain<N>) -> Vec<Request> {
    let mut out = Vec::new();
    let mut infinite = domain.clone();
    infinite.upper = vec![None; N];
    out.extend(request(infinite, "infinite"));
    if let Some(axis) = domain.owner.iter().position(|&active| active) {
        let mut wide = domain.clone();
        wide.upper[axis] = None;
        wide.rank = None;
        wide.powers.max_positive_power = Some(1 << 40);
        wide.powers.max_power_difference = None;
        out.extend(request(wide, "wide"));
        let mut empty = domain.clone();
        empty.powers.max_positive_power = Some(0);
        out.extend(request(empty, "empty"));
    } else {
        let mut empty = domain.clone();
        empty.lower[0] = empty.lower[0].max(1);
        empty.upper[0] = Some(empty.upper[0].unwrap_or(1).max(1));
        empty.rank = Some(0);
        out.extend(request(empty, "empty"));
    }
    let mut saturated = domain.clone();
    saturated.lower[0] = 300;
    saturated.upper[0] = None;
    out.extend(request(saturated, "saturated"));
    out
}

/// Every `stride`-th traced index request (outcome contained or new).
fn traced_requests(path: &str, stride: usize, limit: usize) -> (Vec<Request>, Value) {
    const IMAGE: usize = 34 + 4 * N;
    const RECORD: usize = 28 + IMAGE;
    let mut file = std::io::BufReader::with_capacity(1 << 24, std::fs::File::open(path).unwrap());
    let mut header = [0; 12];
    file.read_exact(&mut header).unwrap();
    assert_eq!(&header[..8], b"RRTRCRD1");
    assert_eq!(
        u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize,
        N
    );
    let mut record = [0_u8; RECORD];
    let (mut records, mut eligible, mut invalid) = (0_usize, 0_usize, 0_usize);
    let mut kinds = [0_usize; 5];
    let mut out = Vec::new();
    while file.read_exact(&mut record).is_ok() {
        records += 1;
        let kind = record[0];
        kinds[usize::from(kind.min(4))] += 1;
        if kind != 2 && kind != 3 {
            continue;
        }
        eligible += 1;
        if eligible % stride != 0 || out.len() >= limit {
            continue;
        }
        let Some(image) = CompactDomain::<N>::from_trace_image(&record[28..]) else {
            invalid += 1;
            continue;
        };
        let label = if kind == 2 {
            "traced_contained"
        } else {
            "traced_new"
        };
        match request(image.expand(), label) {
            Some(r) => out.push(r),
            None => invalid += 1,
        }
    }
    let receipt = json!({"path":path,"records":records,"index_requests":eligible,
        "kinds_exact_orthant_contained_new_refused":kinds,"stride":stride,
        "sampled":out.len(),"invalid":invalid});
    (out, receipt)
}

#[derive(Default)]
struct Tally {
    requests: usize,
    forward_pairs: usize,
    reverse_pairs: usize,
    sweep_forward_pairs: usize,
    sweep_reverse_pairs: usize,
    exact_forward_pairs: usize,
    exact_reverse_pairs: usize,
    forward_true: usize,
    reverse_true: usize,
    mismatches: usize,
    lookup_mismatches: usize,
    by_kind: std::collections::BTreeMap<&'static str, usize>,
    examples: Vec<String>,
}

impl Tally {
    fn bad(&mut self, what: String) {
        self.mismatches += 1;
        if self.examples.len() < 20 {
            self.examples.push(what);
        }
    }
    fn add(&mut self, o: Tally) {
        self.requests += o.requests;
        self.forward_pairs += o.forward_pairs;
        self.reverse_pairs += o.reverse_pairs;
        self.sweep_forward_pairs += o.sweep_forward_pairs;
        self.sweep_reverse_pairs += o.sweep_reverse_pairs;
        self.exact_forward_pairs += o.exact_forward_pairs;
        self.exact_reverse_pairs += o.exact_reverse_pairs;
        self.forward_true += o.forward_true;
        self.reverse_true += o.reverse_true;
        self.mismatches += o.mismatches;
        self.lookup_mismatches += o.lookup_mismatches;
        for (k, v) in o.by_kind {
            *self.by_kind.entry(k).or_default() += v;
        }
        for e in o.examples {
            if self.examples.len() < 20 {
                self.examples.push(e);
            }
        }
    }
}

/// Differential visitor: every logical candidate against the old predicate.
struct Diff<'a> {
    legacy: &'a Legacy<'a, N>,
    stored: Stored<'a, N>,
    query: &'a Query<N>,
    old: &'a legacy::OldQuery<N>,
    reverse: bool,
    checks: usize,
    tally: &'a mut Tally,
}

impl Diff<'_> {
    fn old(&self, id: usize) -> (bool, bool) {
        if self.reverse {
            self.legacy.reverse_verdict(id, self.old)
        } else {
            self.legacy.forward_verdict(id, self.old)
        }
    }
    fn pair(&mut self) {
        if self.reverse {
            self.tally.reverse_pairs += 1;
        } else {
            self.tally.forward_pairs += 1;
        }
    }
}

impl super::super::index::Visit for Diff<'_> {
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        for (j, &id) in run.iter().enumerate() {
            let (old_rejected, old) = self.old(id as usize);
            if old || old_rejected != (word >> j & 1 != 0) {
                let reverse = self.reverse;
                self.tally.bad(format!(
                    "rejected id={id} reverse={reverse} old={old} old_word_rejected={old_rejected}"
                ));
            }
            self.pair();
        }
        self.checks += run.len();
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        let (old_rejected, old) = self.old(id);
        let new = if self.reverse {
            self.stored.contained_by(id, self.query)
        } else {
            self.stored.contains(id, self.query)
        };
        if old_rejected || new != old {
            let reverse = self.reverse;
            self.tally.bad(format!(
                "tested id={id} reverse={reverse} new={new} old={old} old_word_rejected={old_rejected}"
            ));
        }
        if new {
            if self.reverse {
                self.tally.reverse_true += 1;
            } else {
                self.tally.forward_true += 1;
            }
        }
        self.pair();
        self.checks += 1;
        Ok(new)
    }
}

fn probe_of(query: &Query<N>) -> Probe<'_, N> {
    Probe::new(Coordinates::of(&query.core), query.word, query.lanes, true)
}

fn differential(
    queue: &Queue<N>,
    legacy: &Legacy<'_, N>,
    request: &Request,
    sweep: bool,
    tally: &mut Tally,
) {
    tally.requests += 1;
    *tally.by_kind.entry(request.kind).or_default() += 1;
    let Some(bucket) = queue.by_owner.get(&request.key) else {
        return;
    };
    let stored = queue.stored();
    let query = &request.query;
    let probe = probe_of(query);
    let signature = Signature::of(&query.core);
    // Forward: result and charged count against the historical lookup.
    let mut diff = Diff {
        legacy,
        stored,
        query,
        old: &request.old,
        reverse: false,
        checks: 0,
        tally: &mut *tally,
    };
    let found = bucket
        .indexed
        .find_from(signature, &probe, 0, &mut diff)
        .unwrap();
    let checks = diff.checks;
    let (old_found, old_checks) = legacy.find(request.key, &request.old);
    if (found, checks) != (old_found, old_checks) {
        tally.lookup_mismatches += 1;
        tally.bad(format!(
            "forward {} found={found:?}/{old_found:?} checks={checks}/{old_checks}",
            request.kind
        ));
    }
    // Reverse: the helper reverse set and its examined count.
    let mut diff = Diff {
        legacy,
        stored,
        query,
        old: &request.old,
        reverse: true,
        checks: 0,
        tally: &mut *tally,
    };
    let set = bucket
        .indexed
        .collect_contained(signature, &probe, usize::MAX, || Ok(()), &mut diff)
        .unwrap();
    let checks = diff.checks;
    let (old_set, old_checks) = legacy.reverse(request.key, &request.old);
    if (&set, checks) != (&old_set, old_checks) {
        tally.lookup_mismatches += 1;
        tally.bad(format!(
            "reverse {} set={}/{} checks={checks}/{old_checks}",
            request.kind,
            set.len(),
            old_set.len()
        ));
    }
    if !sweep {
        return;
    }
    let exact_query = query.lanes.is_some_and(|lanes| !lanes.lossy);
    bucket
        .indexed
        .sweep(&probe, |id, (fw, fp), (rw, rp), inexact| {
            let (old_rejected, old) = legacy.forward_verdict(id, &request.old);
            if fw == old_rejected || (old && !fp) || (exact_query && !inexact && fp != old) {
                tally.bad(format!(
                    "sweep forward id={id} word={fw} pass={fp} old={old}"
                ));
            }
            tally.sweep_forward_pairs += 1;
            tally.exact_forward_pairs += usize::from(exact_query && !inexact);
            let (old_rejected, old) = legacy.reverse_verdict(id, &request.old);
            if rw == old_rejected || (old && !rp) || (exact_query && !inexact && rp != old) {
                tally.bad(format!(
                    "sweep reverse id={id} word={rw} pass={rp} old={old}"
                ));
            }
            tally.sweep_reverse_pairs += 1;
            tally.exact_reverse_pairs += usize::from(exact_query && !inexact);
        });
}

/// Reverse timing visitor (the helper-prepared reverse set).
struct ReverseCharge<'a> {
    stored: Stored<'a, N>,
    query: &'a Query<N>,
    checks: usize,
    rejections: usize,
}

impl super::super::index::Visit for ReverseCharge<'_> {
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        self.checks += run.len();
        self.rejections += word.count_ones() as usize;
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        self.checks += 1;
        Ok(self.stored.contained_by(id, self.query))
    }
}

#[derive(Default, Clone, Copy)]
struct Arm {
    requests: usize,
    checks: usize,
    hits: usize,
    cpu_ns: u64,
    wait_ns: u64,
    wall_ns: u64,
}

impl Arm {
    fn json(&self) -> Value {
        let per = |x: u64, d: usize| (d > 0).then(|| x as f64 / d as f64);
        json!({"requests":self.requests,"checks":self.checks,"hits":self.hits,
            "cpu_ns":self.cpu_ns,"wait_ns":self.wait_ns,"wall_ns":self.wall_ns,
            "cpu_ns_per_check":per(self.cpu_ns, self.checks),
            "cpu_us_per_request":per(self.cpu_ns, self.requests).map(|v| v / 1000.0),
            "checks_per_request":per(self.checks as u64, self.requests)})
    }
}

fn timed(arm: &mut Arm, run: impl FnOnce(&mut Arm)) {
    let (cpu, wait) = schedstat();
    let started = Instant::now();
    run(arm);
    let wall = started.elapsed().as_nanos() as u64;
    let (cpu2, wait2) = schedstat();
    arm.cpu_ns += cpu2 - cpu;
    arm.wait_ns += wait2 - wait;
    arm.wall_ns += wall;
}

#[test]
#[ignore = "offline gen-7 harness; needs RUSTRED_KERNEL_CKPT (tens of GB)"]
fn gen7_kernel_differential_and_cost() {
    let Some(dir) = std::env::var_os("RUSTRED_KERNEL_CKPT") else {
        println!("RUSTRED_KERNEL_CKPT not set; skipped");
        return;
    };
    let out_path = std::env::var("RUSTRED_KERNEL_OUT").expect("RUSTRED_KERNEL_OUT");
    let threads = env_usize("RUSTRED_KERNEL_THREADS", 16);
    let started = Instant::now();
    let mut phases = serde_json::Map::new();
    let mut mark = |name: &str, since: &mut Instant| {
        phases.insert(name.into(), json!(since.elapsed().as_secs_f64()));
        *since = Instant::now();
    };
    let mut since = Instant::now();
    let sections = read_queue_sections::<N>(std::path::Path::new(&dir)).unwrap();
    let old_buckets: legacy::Buckets = sections.decode_index_as().unwrap();
    let stored_buckets = sections.buckets().unwrap();
    let (metadata, domains) = sections.into_parts();
    mark("decode", &mut since);
    let queue = Queue::<N>::restore_from_parts(
        metadata,
        domains,
        stored_buckets,
        None,
        &mut Phases::default(),
    )
    .unwrap();
    mark("restore_new_layout", &mut since);
    let ids = queue.domains.len();
    // Independent native derivation of every ID's historical word, and the
    // check of the kernel image rebuilt from the immutable summary.
    let chunk = ids.div_ceil(threads);
    #[allow(clippy::type_complexity)]
    let results: Vec<(Vec<u64>, Vec<legacy::Summary<N>>, usize, usize)> =
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..threads)
                .map(|t| {
                    let queue = &queue;
                    scope.spawn(move || {
                        let range = (t * chunk).min(ids)..((t + 1) * chunk).min(ids);
                        let (mut words, mut bad, mut wide) =
                            (Vec::with_capacity(range.len()), 0, 0);
                        let mut old = Vec::with_capacity(range.len());
                        for id in range {
                            let native = queue.domains[id].native_summary();
                            let word = super::super::bits::word(&native);
                            let (derived, lanes) = super::super::compact::stored_image(
                                &queue.domains[id],
                                &queue.summaries[id],
                            )
                            .unwrap();
                            wide += usize::from(queue.summaries[id].is_wide());
                            bad += usize::from(
                                derived != word
                                    || lanes != super::super::index::Lanes::of_core(&native)
                                    || queue.summaries[id] != CompactSummary::from_core(&native),
                            );
                            words.push(word);
                            old.push(legacy::Summary::from_core(&native));
                        }
                        (words, old, bad, wide)
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
    let mut words = Vec::with_capacity(ids);
    let mut historical = Vec::with_capacity(ids);
    let (mut image_mismatches, mut wide_summaries) = (0, 0);
    for (w, old, bad, wide) in results {
        words.extend(w);
        historical.extend(old);
        image_mismatches += bad;
        wide_summaries += wide;
    }
    let mut live = vec![false; ids];
    let mut buckets = HashMap::new();
    for (phase, owner, bucket) in old_buckets.0 {
        let owner: [bool; N] = owner.try_into().unwrap();
        bucket.indexed.for_each_id(|id| live[id] = true);
        buckets.insert((phase, owner), bucket.indexed);
    }
    let legacy = Legacy {
        domains: &queue.domains,
        slab: legacy::SummarySlab::restore(
            historical
                .into_iter()
                .zip(&live)
                .map(|(summary, &live)| live.then_some(summary)),
            ids,
        ),
        bits: words,
        buckets,
    };
    let live_count = live.iter().filter(|&&l| l).count();
    assert_eq!(live_count, queue.containment_candidate_count());
    for id in 0..ids {
        assert_eq!(queue.is_indexed(id), live[id], "index membership of {id}");
    }
    drop(live);
    mark("restore_old_layout_and_image_check", &mut since);
    let storage = queue.storage_json();

    // Requests: real traced index requests plus synthetic variants.
    let stride = env_usize("RUSTRED_KERNEL_STRIDE", 400);
    let limit = env_usize("RUSTRED_KERNEL_LIMIT", 60_000);
    let (mut requests, trace) = match std::env::var("RUSTRED_KERNEL_TRACE") {
        Ok(path) => traced_requests(&path, stride, limit),
        Err(_) => (Vec::new(), json!(null)),
    };
    let traced = requests.len();
    // Admitted domains as requests too (live and retired), for coverage.
    let mut state = 0x9E37_79B9_7F4A_7C15_u64;
    for _ in 0..limit / 4 {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
        let id = (state >> 17) as usize % ids;
        let domain = queue.domains[id].expand();
        requests.extend(variants(&domain));
        requests.extend(request(domain, "admitted"));
    }
    mark("requests", &mut since);

    // Differential, parallel over requests.
    let sweep_every = env_usize("RUSTRED_KERNEL_SWEEP", 20).max(1);
    let per = requests.len().div_ceil(threads).max(1);
    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = requests
            .chunks(per)
            .enumerate()
            .map(|(c, chunk)| {
                let (queue, legacy) = (&queue, &legacy);
                scope.spawn(move || {
                    let mut tally = Tally::default();
                    for (i, request) in chunk.iter().enumerate() {
                        let sweep = (c * per + i) % sweep_every == 0;
                        differential(queue, legacy, request, sweep, &mut tally);
                    }
                    tally
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().unwrap()).collect()
    });
    let mut tally = Tally::default();
    for t in tallies {
        tally.add(t);
    }
    mark("differential", &mut since);

    // Timing: one thread, interleaved per chunk, traced requests first.
    let timing = env_usize("RUSTRED_KERNEL_TIMING", 20_000).min(requests.len());
    let timed_requests: Vec<&Request> = if traced > 0 {
        requests[..traced].iter().take(timing).collect()
    } else {
        requests.iter().take(timing).collect()
    };
    let cpu = current_cpu();
    let jiffies_before = cpu.and_then(cpu_jiffies);
    let own_before = schedstat().0;
    let (mut old_fwd, mut new_fwd, mut old_rev, mut new_rev) = (
        Arm::default(),
        Arm::default(),
        Arm::default(),
        Arm::default(),
    );
    let (mut old_fwd_hit, mut new_fwd_hit, mut old_fwd_miss, mut new_fwd_miss) = (
        Arm::default(),
        Arm::default(),
        Arm::default(),
        Arm::default(),
    );
    let stored = queue.stored();
    for repeat in 0..2 {
        for (c, chunk) in timed_requests.chunks(250).enumerate() {
            let new_first = (c + repeat) % 2 == 1;
            for arm in 0..2 {
                if (arm == 0) == new_first {
                    timed(&mut new_fwd, |a| {
                        for r in chunk {
                            let Some(bucket) = queue.by_owner.get(&r.key) else {
                                continue;
                            };
                            let (mut checks, mut session) = (0, SessionCounters::default());
                            let probe = Probe::new(
                                Coordinates::of(&r.query.core),
                                r.query.word,
                                r.query.lanes,
                                true,
                            );
                            let found = bucket
                                .indexed
                                .find_from(
                                    Signature::of(&r.query.core),
                                    &probe,
                                    0,
                                    &mut Charged {
                                        checks: &mut checks,
                                        session: &mut session,
                                        stored,
                                        query: &r.query,
                                    },
                                )
                                .unwrap();
                            a.requests += 1;
                            a.checks += checks;
                            a.hits += usize::from(found.is_some());
                        }
                    });
                } else {
                    timed(&mut old_fwd, |a| {
                        for r in chunk {
                            let (found, checks) = legacy.find(r.key, &r.old);
                            a.requests += 1;
                            a.checks += checks;
                            a.hits += usize::from(found.is_some());
                        }
                    });
                }
            }
            for arm in 0..2 {
                if (arm == 0) == new_first {
                    timed(&mut new_rev, |a| {
                        for r in chunk {
                            let Some(bucket) = queue.by_owner.get(&r.key) else {
                                continue;
                            };
                            let mut visit = ReverseCharge {
                                stored,
                                query: &r.query,
                                checks: 0,
                                rejections: 0,
                            };
                            let set = bucket
                                .indexed
                                .collect_contained(
                                    Signature::of(&r.query.core),
                                    &probe_of(&r.query),
                                    usize::MAX,
                                    || Ok(()),
                                    &mut visit,
                                )
                                .unwrap();
                            a.requests += 1;
                            a.checks += visit.checks;
                            a.hits += set.len();
                        }
                    });
                } else {
                    timed(&mut old_rev, |a| {
                        for r in chunk {
                            let (set, checks) = legacy.reverse(r.key, &r.old);
                            a.requests += 1;
                            a.checks += checks;
                            a.hits += set.len();
                        }
                    });
                }
            }
        }
    }
    // Per-outcome forward costs (hits vs misses), one request at a time.
    for r in timed_requests.iter().take(timing / 2) {
        let hit = legacy.find(r.key, &r.old).0.is_some();
        let (old_arm, new_arm) = if hit {
            (&mut old_fwd_hit, &mut new_fwd_hit)
        } else {
            (&mut old_fwd_miss, &mut new_fwd_miss)
        };
        timed(old_arm, |a| {
            let (_, checks) = legacy.find(r.key, &r.old);
            a.requests += 1;
            a.checks += checks;
        });
        timed(new_arm, |a| {
            if let Some(bucket) = queue.by_owner.get(&r.key) {
                let (mut checks, mut session) = (0, SessionCounters::default());
                bucket
                    .indexed
                    .find_from(
                        Signature::of(&r.query.core),
                        &probe_of(&r.query),
                        0,
                        &mut Charged {
                            checks: &mut checks,
                            session: &mut session,
                            stored,
                            query: &r.query,
                        },
                    )
                    .unwrap();
                a.checks += checks;
            }
            a.requests += 1;
        });
    }
    let own = schedstat().0 - own_before;
    let foreign = match (cpu, jiffies_before, cpu.and_then(cpu_jiffies)) {
        (Some(cpu), Some((b0, t0)), Some((b1, t1))) => {
            let busy_ns = (b1 - b0) as f64 * 1e7;
            let total_ns = (t1 - t0) as f64 * 1e7;
            json!({"cpu":cpu,"busy_ns":busy_ns,"own_ns":own,"window_ns":total_ns,
                "foreign_share":((busy_ns - own as f64).max(0.0) / total_ns.max(1.0))})
        }
        _ => json!(null),
    };
    mark("timing", &mut since);
    let ratio = |old: &Arm, new: &Arm| {
        let o = old.cpu_ns as f64 / old.checks.max(1) as f64;
        let n = new.cpu_ns as f64 / new.checks.max(1) as f64;
        (n > 0.0).then(|| o / n)
    };
    let receipt = json!({
        "checkpoint": dir.to_string_lossy(),
        "arity": N,
        "admitted_ids": ids,
        "live_candidates": queue.containment_candidate_count(),
        "wide_summaries": wide_summaries,
        "image_mismatches": image_mismatches,
        "storage": storage,
        "trace": trace,
        "requests": {"total": requests.len(), "traced": traced, "by_kind": tally.by_kind},
        "differential": {
            "forward_pairs": tally.forward_pairs, "reverse_pairs": tally.reverse_pairs,
            "sweep_forward_pairs": tally.sweep_forward_pairs,
            "sweep_reverse_pairs": tally.sweep_reverse_pairs,
            "exact_forward_pairs": tally.exact_forward_pairs,
            "exact_reverse_pairs": tally.exact_reverse_pairs,
            "forward_contained": tally.forward_true, "reverse_contained": tally.reverse_true,
            "mismatches": tally.mismatches, "lookup_mismatches": tally.lookup_mismatches,
            "examples": tally.examples,
        },
        "timing": {
            "unit": "cpu_ns_per_check = thread on-CPU ns / logical candidates (containment_checks units; reverse: examined candidates)",
            "timed_requests": timed_requests.len(), "repeats": 2, "chunk": 250,
            "old_forward": old_fwd.json(), "new_forward": new_fwd.json(),
            "old_reverse": old_rev.json(), "new_reverse": new_rev.json(),
            "old_forward_hit": old_fwd_hit.json(), "new_forward_hit": new_fwd_hit.json(),
            "old_forward_miss": old_fwd_miss.json(), "new_forward_miss": new_fwd_miss.json(),
            "forward_cpu_per_check_ratio_old_over_new": ratio(&old_fwd, &new_fwd),
            "reverse_cpu_per_check_ratio_old_over_new": ratio(&old_rev, &new_rev),
            "hit_ratio": ratio(&old_fwd_hit, &new_fwd_hit),
            "miss_ratio": ratio(&old_fwd_miss, &new_fwd_miss),
            "foreign_load": foreign,
        },
        "phases_seconds": phases,
        "total_seconds": started.elapsed().as_secs_f64(),
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&receipt).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&receipt).unwrap());
    assert_eq!(image_mismatches, 0);
    assert_eq!(tally.mismatches, 0, "{:?}", tally.examples);
    assert_eq!(old_fwd.checks, new_fwd.checks);
    assert_eq!(old_rev.checks, new_rev.checks);
}
