//! Resolution pipeline of master plan §3.2 evaluated on real successor
//! streams: for every request of an inspection, the first tier that resolves
//! it. Tiers (in order): job-local exact set (unbounded), self-scope (request
//! inside the inspected parent, same bucket), Local (inside an earlier miss of
//! the same inspection, same bucket), per-job MRU of verified targets (k),
//! exact store probe, helpers/orthants, then the layers (hit) or a miss. The
//! ground-truth outcome of each request is the engine's committed admission.
use crate::ckpt::{Raw, T};
use crate::trace::{self, Ev, Job};
use std::collections::{HashMap, HashSet};

pub const TIERS: [&str; 10] = [
    "exact-job",
    "self",
    "local",
    "mru",
    "exact-store",
    "helper",
    "layer-hit",
    "miss",
    "frontier",
    "unjoined",
];
pub const EXACT_JOB: usize = 0;
pub const SELF: usize = 1;
pub const LOCAL: usize = 2;
pub const MRU: usize = 3;
pub const EXACT_STORE: usize = 4;
pub const HELPER: usize = 5;
pub const LAYER_HIT: usize = 6;
pub const MISS: usize = 7;
pub const FRONTIER: usize = 8;
pub const UNJOINED: usize = 9;

#[derive(Default, Clone, Debug)]
pub struct Tally {
    pub requests: [u64; 10],
    /// Containment tests spent in the Local / MRU / helper tiers.
    pub local_tests: u64,
    pub mru_tests: u64,
    pub helper_tests: u64,
    pub jobs: u64,
    pub join_mismatch: u64,
}
impl Tally {
    pub fn add(&mut self, o: &Tally) {
        for i in 0..10 {
            self.requests[i] += o.requests[i];
        }
        self.local_tests += o.local_tests;
        self.mru_tests += o.mru_tests;
        self.helper_tests += o.helper_tests;
        self.jobs += o.jobs;
        self.join_mismatch += o.join_mismatch;
    }
    pub fn total(&self) -> u64 {
        self.requests.iter().sum()
    }
    pub fn cheap(&self) -> u64 {
        self.requests[..=HELPER].iter().sum()
    }
}

/// Committed outcome of one admission request.
#[derive(Clone, Copy, Debug)]
pub struct Outcome {
    pub kind: u8,
    pub target: u32,
    /// IDs admitted before this request was committed (exact-store and
    /// MRU/helper targets must be older than this).
    pub watermark: u32,
    /// Index of the committed coordinator record.
    pub rec: u32,
}

pub struct Ctx<'a> {
    /// Tight summary of any admitted ID (including post-snapshot IDs).
    pub t_of: &'a dyn Fn(u32) -> Option<T>,
    /// Exact-store probe: an admitted ID with this raw image.
    pub exact: &'a dyn Fn(&Raw) -> Option<u32>,
    /// Helper/orthant candidates of a (phase, owner) bucket.
    pub helpers: &'a HashMap<(u8, u16), Vec<u32>>,
}

/// Per-request result: tier, and for layer-reaching requests the outcome.
pub struct Classified {
    pub tier: usize,
    pub count: u64,
    pub t: Option<T>,
    pub outcome: Option<Outcome>,
}

/// Classify one job's requests. `outcomes[i]` is the committed outcome of the
/// i-th Admit event (None when the job could not be joined).
pub fn classify(job: &Job, outcomes: Option<&[Outcome]>, ctx: &Ctx, k: usize, tally: &mut Tally, out: &mut Vec<Classified>) {
    let parent = T::project(&job.parent);
    let pkey = (job.parent.phase, job.parent.owner);
    let mut seen: HashSet<Raw> = HashSet::new();
    let mut local: Vec<((u8, u16), T)> = Vec::new();
    let mut mru: Vec<((u8, u16), u32, T)> = Vec::new();
    let mut ai = 0usize;
    tally.jobs += 1;
    for ev in &job.events {
        match ev {
            Ev::KnownReuse { count } => {
                tally.requests[EXACT_JOB] += *count as u64;
                out.push(Classified { tier: EXACT_JOB, count: *count as u64, t: None, outcome: None });
            }
            Ev::PreAdmitted { count, .. } => {
                tally.requests[HELPER] += *count as u64;
                out.push(Classified { tier: HELPER, count: *count as u64, t: None, outcome: None });
            }
            Ev::Frontier { count } | Ev::Unrepresentable { count } => {
                tally.requests[FRONTIER] += *count as u64;
            }
            Ev::Admit { raw, count, .. } => {
                let count = *count as u64;
                let outcome = outcomes.and_then(|o| o.get(ai).copied());
                ai += 1;
                let q = T::project(raw);
                let key = (raw.phase, raw.owner);
                let mut tier = None;
                if !seen.insert(*raw) {
                    tier = Some(EXACT_JOB);
                } else if key == pkey && parent.contains(&q) {
                    tier = Some(SELF);
                } else {
                    for (b, c) in &local {
                        if *b == key {
                            tally.local_tests += 1;
                            if c.contains(&q) {
                                tier = Some(LOCAL);
                                break;
                            }
                        }
                    }
                    if tier.is_none() {
                        let mut tested = 0;
                        let mut hit = None;
                        for (pos, (b, _, c)) in mru.iter().enumerate() {
                            if *b != key {
                                continue;
                            }
                            if tested == k {
                                break;
                            }
                            tested += 1;
                            tally.mru_tests += 1;
                            if c.contains(&q) {
                                hit = Some(pos);
                                break;
                            }
                        }
                        if let Some(pos) = hit {
                            let e = mru.remove(pos);
                            mru.insert(0, e);
                            tier = Some(MRU);
                        }
                    }
                    let push_target = |target: u32, mru: &mut Vec<((u8, u16), u32, T)>| {
                        if let Some(c) = (ctx.t_of)(target) {
                            if let Some(pos) = mru.iter().position(|e| e.1 == target) {
                                mru.remove(pos);
                            }
                            mru.insert(0, (key, target, c));
                            if mru.len() > 4 * k.max(16) {
                                mru.pop();
                            }
                        }
                    };
                    if tier.is_none() {
                        if let Some(target) = (ctx.exact)(raw).filter(|&t| outcome.is_none_or(|o| t < o.watermark)) {
                            tier = Some(EXACT_STORE);
                            push_target(target, &mut mru);
                        }
                    }
                    if tier.is_none() {
                        if let Some(hs) = ctx.helpers.get(&key) {
                            for &h in hs {
                                tally.helper_tests += 1;
                                if (ctx.t_of)(h).is_some_and(|c| c.contains(&q)) {
                                    tier = Some(HELPER);
                                    push_target(h, &mut mru);
                                    break;
                                }
                            }
                        }
                    }
                    if tier.is_none() {
                        tier = Some(match outcome {
                            None => UNJOINED,
                            Some(o) => match o.kind {
                                trace::NEW => {
                                    local.push((key, q));
                                    MISS
                                }
                                trace::CONTAINED | trace::EXACT => {
                                    push_target(o.target, &mut mru);
                                    LAYER_HIT
                                }
                                trace::ORTHANT => {
                                    push_target(o.target, &mut mru);
                                    HELPER
                                }
                                _ => UNJOINED,
                            },
                        });
                    }
                }
                let tier = tier.unwrap();
                tally.requests[tier] += count;
                out.push(Classified { tier, count, t: Some(q), outcome });
            }
        }
    }
}

/// Group coordinator records by source, in commit order.
pub fn by_source(recs: &[trace::Admission]) -> HashMap<u32, Vec<usize>> {
    let mut m: HashMap<u32, Vec<usize>> = HashMap::new();
    for (i, r) in recs.iter().enumerate() {
        m.entry(r.source).or_default().push(i);
    }
    m
}

/// Join a job's Admit events to the committed records of its source.
/// Returns None if counts or images disagree (e.g. a cancelled duplicate).
pub fn join(job: &Job, recs: &[trace::Admission], wm: &[u32], idx: Option<&Vec<usize>>) -> Option<Vec<Outcome>> {
    let admits: Vec<&Raw> = job
        .events
        .iter()
        .filter_map(|e| match e {
            Ev::Admit { raw, .. } => Some(raw),
            _ => None,
        })
        .collect();
    if admits.is_empty() {
        return Some(Vec::new());
    }
    let idx = idx?;
    if admits.len() != idx.len() {
        return None;
    }
    let mut out = Vec::with_capacity(idx.len());
    for (raw, &i) in admits.iter().zip(idx) {
        let r = &recs[i];
        if r.raw != **raw {
            return None;
        }
        out.push(Outcome { kind: r.kind, target: r.target, watermark: wm[i], rec: i as u32 });
    }
    Some(out)
}

/// ID watermark before each coordinator record: `base` plus the number of
/// domains created earlier (new IDs are dense and increasing).
pub fn watermarks(recs: &[trace::Admission], base: u32) -> Vec<u32> {
    let mut wm = Vec::with_capacity(recs.len());
    let mut next = base;
    for r in recs {
        wm.push(next);
        if r.kind == trace::NEW {
            next = next.max(r.target + 1);
        }
    }
    wm
}
