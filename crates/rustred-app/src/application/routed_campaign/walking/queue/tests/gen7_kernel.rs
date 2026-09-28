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
//!    sides are unsaturated); sweeps are chosen per request kind (every k-th
//!    request of each kind) and every count is reported per kind;
//! 3. timing, with the test-only index work counters switched off so that
//!    both arms run their production instructions: (a) one thread pinned to
//!    one CPU, forward and reverse lookups, old/new interleaved per chunk;
//!    (b) the same on `RUSTRED_KERNEL_MT_THREADS` threads pinned to distinct
//!    CPUs, all threads on the same arm between barriers. CPU from
//!    `/proc/thread-self/schedstat`; a load recorder per phase (foreign busy
//!    time on the pinned CPU and on the whole mask, run-queue delay) whose
//!    verdict is INVALID, not clamped, when the accounting cannot hold;
//! 4. the cost of the coordinator's `queue_storage` telemetry at gen-7 shape:
//!    the O(blocks) walk of d9163195 against the running totals.
//!
//! Environment: `RUSTRED_KERNEL_CKPT` (checkpoint directory),
//! `RUSTRED_KERNEL_OUT` (JSON receipt), optional `RUSTRED_KERNEL_TRACE`
//! (`coord-*.bin` of the admission trace), `RUSTRED_KERNEL_STRIDE` (sample
//! every k-th traced index request, default 400), `RUSTRED_KERNEL_LIMIT`
//! (requests, default 60000), `RUSTRED_KERNEL_SWEEP` (sweep every k-th
//! request of each kind, default 20), `RUSTRED_KERNEL_THREADS` (differential
//! threads, default 16), `RUSTRED_KERNEL_TIMING` (timed requests, default
//! 20000), `RUSTRED_KERNEL_TIMING_CPU` (default: the last CPU of the mask),
//! `RUSTRED_KERNEL_MT_THREADS` (default 8, 0 disables; capped by the mask),
//! `RUSTRED_KERNEL_PHASE_FILE` (optional: the current phase name, for an
//! external profiler), and provenance stamped into the receipt:
//! `RUSTRED_KERNEL_BINARY_SHA256`, `RUSTRED_KERNEL_GIT_HEAD`,
//! `RUSTRED_KERNEL_GIT_STATUS` (the test also hashes itself with blake3).
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

/// The CPU list of a `Cpus_allowed_list` line (`/proc/self/status` for the
/// process mask, `/proc/thread-self/status` for this thread).
fn allowed_cpus(status: &str) -> Vec<usize> {
    let text = std::fs::read_to_string(status).unwrap_or_default();
    let Some(list) = text
        .lines()
        .find_map(|line| line.strip_prefix("Cpus_allowed_list:"))
    else {
        return Vec::new();
    };
    let mut cpus = Vec::new();
    for part in list.trim().split(',') {
        match part.split_once('-') {
            Some((a, b)) => {
                if let (Ok(a), Ok(b)) = (a.parse::<usize>(), b.parse::<usize>()) {
                    cpus.extend(a..=b);
                }
            }
            None => cpus.extend(part.parse::<usize>().ok()),
        }
    }
    cpus
}

/// Pin the calling thread to `cpu` without unsafe code (`taskset` on its
/// TID). True only when the kernel then reports exactly that one CPU.
fn pin_thread(cpu: usize) -> bool {
    let Some(tid) = std::fs::read_link("/proc/thread-self")
        .ok()
        .and_then(|path| path.file_name()?.to_str()?.parse::<u64>().ok())
    else {
        return false;
    };
    let pinned = ["taskset", "/run/current-system/sw/bin/taskset"]
        .iter()
        .any(|taskset| {
            std::process::Command::new(taskset)
                .args(["-p", "-c", &cpu.to_string(), &tid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        });
    pinned && allowed_cpus("/proc/thread-self/status") == [cpu]
}

/// (busy, total) jiffies of every CPU, from one read of `/proc/stat`; busy
/// is user + nice + system + irq + softirq + steal.
fn cpu_jiffies() -> HashMap<usize, (u64, u64)> {
    let text = std::fs::read_to_string("/proc/stat").unwrap_or_default();
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("cpu")?;
            let (cpu, fields) = rest.split_once(' ')?;
            let cpu = cpu.parse::<usize>().ok()?;
            let v: Vec<u64> = fields
                .split_whitespace()
                .map(|x| x.parse().unwrap_or(0))
                .collect();
            let total: u64 = v.iter().take(8).sum();
            Some((cpu, (total - v[3] - v[4], total)))
        })
        .collect()
}

/// This process's user + system CPU in clock ticks (`/proc/self/stat`,
/// exited threads included).
fn process_ticks() -> u64 {
    let text = std::fs::read_to_string("/proc/self/stat").unwrap_or_default();
    let fields: Vec<u64> = text
        .rfind(')')
        .map(|end| {
            text[end + 2..]
                .split_whitespace()
                .map(|v| v.parse().unwrap_or(0))
                .collect()
        })
        .unwrap_or_default();
    // After the command: state is field 3, utime 14 and stime 15.
    fields.get(11).copied().unwrap_or(0) + fields.get(12).copied().unwrap_or(0)
}

/// Jiffy and clock-tick length (USER_HZ = 100 on this host's kernels).
const TICK_NS: f64 = 1e7;

/// Load recorder of one timing phase: foreign busy time on the pinned CPU
/// (single thread) and on the whole CPU set, and the timing threads' own
/// run-queue delay. Tick accounting is coarse, so the pinned-CPU check
/// allows 2% of the own time plus two ticks; below that the verdict is
/// INVALID (the thread did not stay on the CPU, or the accounting differs),
/// never a clamped zero.
struct LoadWindow {
    cpus: Vec<usize>,
    jiffies: HashMap<usize, (u64, u64)>,
    ticks: u64,
    started: Instant,
}

impl LoadWindow {
    fn begin(cpus: &[usize]) -> Self {
        Self {
            cpus: cpus.to_vec(),
            jiffies: cpu_jiffies(),
            ticks: process_ticks(),
            started: Instant::now(),
        }
    }

    /// `own_ns`, `run_delay_ns`: the timing threads' schedstat deltas;
    /// `pinned`: the single-thread CPU when pinning succeeded.
    fn end(self, own_ns: u64, run_delay_ns: u64, pinned: Option<usize>) -> Value {
        let wall_ns = self.started.elapsed().as_nanos() as f64;
        let after = cpu_jiffies();
        let ticks = process_ticks().saturating_sub(self.ticks) as f64 * TICK_NS;
        let delta = |cpu: &usize| {
            let (b0, t0) = self.jiffies.get(cpu).copied().unwrap_or_default();
            let (b1, t1) = after.get(cpu).copied().unwrap_or_default();
            (
                b1.saturating_sub(b0) as f64 * TICK_NS,
                t1.saturating_sub(t0) as f64 * TICK_NS,
            )
        };
        let busy: f64 = self.cpus.iter().map(|cpu| delta(cpu).0).sum();
        let pinned_cpu = pinned.map(|cpu| {
            let (busy, window) = delta(&cpu);
            let own = own_ns as f64;
            let valid = busy + 0.02 * own + 2.0 * TICK_NS >= own;
            json!({"cpu":cpu,"busy_ns":busy,"own_thread_ns":own_ns,"window_ns":window,
                "verdict": if valid { "VALID" } else { "INVALID" },
                "foreign_share": valid.then(|| (busy - own).max(0.0) / window.max(1.0)),
                "reason": (!valid).then_some("busy < own thread time on the pinned CPU")})
        });
        json!({
            "cpus": self.cpus,
            "wall_ns": wall_ns,
            "pinned": pinned.is_some(),
            "pinned_cpu": pinned_cpu,
            "mask_busy_cpus": busy / wall_ns.max(1.0),
            "own_process_cpus": ticks / wall_ns.max(1.0),
            "foreign_busy_cpus_on_mask": ((busy - ticks) / wall_ns.max(1.0)),
            "own_run_delay_ns": run_delay_ns,
            "own_run_delay_share_of_own_cpu": run_delay_ns as f64 / (own_ns as f64).max(1.0),
            "method": "/proc/stat busy jiffies over the CPU set minus this process's utime+stime (both 10 ms ticks); run delay = schedstat wait of the timing threads",
        })
    }
}

/// Current phase for an external profiler (`RUSTRED_KERNEL_PHASE_FILE`).
fn phase(name: &str) {
    if let Ok(path) = std::env::var("RUSTRED_KERNEL_PHASE_FILE") {
        let _ = std::fs::write(path, name);
    }
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
    by_kind: std::collections::BTreeMap<&'static str, KindTally>,
    examples: Vec<String>,
}

/// The differential counts of one request kind.
#[derive(Default, Clone, Copy)]
struct KindTally {
    requests: usize,
    sweeps: usize,
    forward_pairs: usize,
    reverse_pairs: usize,
    sweep_forward_pairs: usize,
    sweep_reverse_pairs: usize,
    exact_forward_pairs: usize,
    exact_reverse_pairs: usize,
    forward_contained: usize,
    reverse_contained: usize,
}

impl KindTally {
    fn add(&mut self, o: &Self) {
        self.requests += o.requests;
        self.sweeps += o.sweeps;
        self.forward_pairs += o.forward_pairs;
        self.reverse_pairs += o.reverse_pairs;
        self.sweep_forward_pairs += o.sweep_forward_pairs;
        self.sweep_reverse_pairs += o.sweep_reverse_pairs;
        self.exact_forward_pairs += o.exact_forward_pairs;
        self.exact_reverse_pairs += o.exact_reverse_pairs;
        self.forward_contained += o.forward_contained;
        self.reverse_contained += o.reverse_contained;
    }
    /// The global counters' growth since `before`.
    fn since(tally: &Tally, before: &Tally) -> Self {
        Self {
            requests: 1,
            sweeps: 0,
            forward_pairs: tally.forward_pairs - before.forward_pairs,
            reverse_pairs: tally.reverse_pairs - before.reverse_pairs,
            sweep_forward_pairs: tally.sweep_forward_pairs - before.sweep_forward_pairs,
            sweep_reverse_pairs: tally.sweep_reverse_pairs - before.sweep_reverse_pairs,
            exact_forward_pairs: tally.exact_forward_pairs - before.exact_forward_pairs,
            exact_reverse_pairs: tally.exact_reverse_pairs - before.exact_reverse_pairs,
            forward_contained: tally.forward_true - before.forward_true,
            reverse_contained: tally.reverse_true - before.reverse_true,
        }
    }
    fn json(&self) -> Value {
        json!({"requests":self.requests,"sweeps":self.sweeps,
            "forward_pairs":self.forward_pairs,"reverse_pairs":self.reverse_pairs,
            "sweep_forward_pairs":self.sweep_forward_pairs,
            "sweep_reverse_pairs":self.sweep_reverse_pairs,
            "exact_forward_pairs":self.exact_forward_pairs,
            "exact_reverse_pairs":self.exact_reverse_pairs,
            "forward_contained":self.forward_contained,
            "reverse_contained":self.reverse_contained})
    }
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
            self.by_kind.entry(k).or_default().add(&v);
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
    let before = Tally {
        forward_pairs: tally.forward_pairs,
        reverse_pairs: tally.reverse_pairs,
        sweep_forward_pairs: tally.sweep_forward_pairs,
        sweep_reverse_pairs: tally.sweep_reverse_pairs,
        exact_forward_pairs: tally.exact_forward_pairs,
        exact_reverse_pairs: tally.exact_reverse_pairs,
        forward_true: tally.forward_true,
        reverse_true: tally.reverse_true,
        ..Tally::default()
    };
    tally.requests += 1;
    differential_pairs(queue, legacy, request, sweep, tally);
    let mut kind = KindTally::since(tally, &before);
    kind.sweeps = usize::from(sweep && queue.by_owner.contains_key(&request.key));
    tally.by_kind.entry(request.kind).or_default().add(&kind);
}

fn differential_pairs(
    queue: &Queue<N>,
    legacy: &Legacy<'_, N>,
    request: &Request,
    sweep: bool,
    tally: &mut Tally,
) {
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
    tests: usize,
}

impl super::super::index::Visit for ReverseCharge<'_> {
    fn rejected(&mut self, run: &[u32], word: u32) -> Result<(), &'static str> {
        self.checks += run.len();
        self.rejections += word.count_ones() as usize;
        Ok(())
    }
    fn test(&mut self, id: usize) -> Result<bool, &'static str> {
        self.checks += 1;
        self.tests += 1;
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

impl Arm {
    fn add(&mut self, o: &Self) {
        self.requests += o.requests;
        self.checks += o.checks;
        self.hits += o.hits;
        self.cpu_ns += o.cpu_ns;
        self.wait_ns += o.wait_ns;
        self.wall_ns += o.wall_ns;
    }
}

/// Filter breakdown of the timed new-kernel reverse arm.
#[derive(Default, Clone, Copy)]
struct ReverseFilter {
    candidates: usize,
    words: usize,
    tests: usize,
}

/// The four timed arms: forward and reverse lookups of one chunk on the new
/// kernel (the production visitors) and on the historical read paths.
struct Arms<'a> {
    queue: &'a Queue<N>,
    legacy: &'a Legacy<'a, N>,
}

impl Arms<'_> {
    fn forward_new(&self, chunk: &[&Request], a: &mut Arm, filter: &mut SessionCounters) {
        let stored = self.queue.stored();
        for r in chunk {
            // Counted like the historical arm, which also sees absent buckets.
            a.requests += 1;
            let Some(bucket) = self.queue.by_owner.get(&r.key) else {
                continue;
            };
            let mut checks = 0;
            let found = bucket
                .indexed
                .find_from(
                    Signature::of(&r.query.core),
                    &probe_of(&r.query),
                    0,
                    &mut Charged {
                        checks: &mut checks,
                        session: &mut *filter,
                        stored,
                        query: &r.query,
                    },
                )
                .unwrap();
            a.checks += checks;
            a.hits += usize::from(found.is_some());
        }
    }

    fn forward_old(&self, chunk: &[&Request], a: &mut Arm) {
        for r in chunk {
            let (found, checks) = self.legacy.find(r.key, &r.old);
            a.requests += 1;
            a.checks += checks;
            a.hits += usize::from(found.is_some());
        }
    }

    fn reverse_new(&self, chunk: &[&Request], a: &mut Arm, filter: &mut ReverseFilter) {
        let stored = self.queue.stored();
        for r in chunk {
            a.requests += 1;
            let Some(bucket) = self.queue.by_owner.get(&r.key) else {
                continue;
            };
            let mut visit = ReverseCharge {
                stored,
                query: &r.query,
                checks: 0,
                rejections: 0,
                tests: 0,
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
            a.checks += visit.checks;
            a.hits += set.len();
            filter.candidates += visit.checks;
            filter.words += visit.rejections;
            filter.tests += visit.tests;
        }
    }

    fn reverse_old(&self, chunk: &[&Request], a: &mut Arm) {
        for r in chunk {
            let (set, checks) = self.legacy.reverse(r.key, &r.old);
            a.requests += 1;
            a.checks += checks;
            a.hits += set.len();
        }
    }
}

/// Old/new forward and reverse arms plus the new kernel's filter breakdown.
#[derive(Default, Clone, Copy)]
struct Timing {
    old_forward: Arm,
    new_forward: Arm,
    old_reverse: Arm,
    new_reverse: Arm,
    forward_filter: SessionCounters,
    reverse_filter: ReverseFilter,
}

impl Timing {
    /// One round of chunk `c`: forward then reverse, old and new each, the
    /// arm order alternating with `c + repeat`. `sync` runs before every arm
    /// (a barrier in the concurrent phase).
    fn round(
        &mut self,
        arms: &Arms<'_>,
        chunk: &[&Request],
        c: usize,
        repeat: usize,
        sync: &dyn Fn(),
    ) {
        let new_first = (c + repeat) % 2 == 1;
        for arm in 0..2 {
            sync();
            if (arm == 0) == new_first {
                let filter = &mut self.forward_filter;
                timed(&mut self.new_forward, |a| {
                    arms.forward_new(chunk, a, filter)
                });
            } else {
                timed(&mut self.old_forward, |a| arms.forward_old(chunk, a));
            }
        }
        for arm in 0..2 {
            sync();
            if (arm == 0) == new_first {
                let filter = &mut self.reverse_filter;
                timed(&mut self.new_reverse, |a| {
                    arms.reverse_new(chunk, a, filter)
                });
            } else {
                timed(&mut self.old_reverse, |a| arms.reverse_old(chunk, a));
            }
        }
    }

    fn add(&mut self, o: &Self) {
        self.old_forward.add(&o.old_forward);
        self.new_forward.add(&o.new_forward);
        self.old_reverse.add(&o.old_reverse);
        self.new_reverse.add(&o.new_reverse);
        let (f, g) = (&mut self.forward_filter, &o.forward_filter);
        f.forward_callbacks += g.forward_callbacks;
        f.forward_bit_rejections += g.forward_bit_rejections;
        f.forward_tests += g.forward_tests;
        self.reverse_filter.candidates += o.reverse_filter.candidates;
        self.reverse_filter.words += o.reverse_filter.words;
        self.reverse_filter.tests += o.reverse_filter.tests;
    }

    /// Own CPU and run delay of the timing threads.
    fn own(&self) -> (u64, u64) {
        let arms = [
            &self.old_forward,
            &self.new_forward,
            &self.old_reverse,
            &self.new_reverse,
        ];
        (
            arms.iter().map(|a| a.cpu_ns).sum(),
            arms.iter().map(|a| a.wait_ns).sum(),
        )
    }

    fn json(&self) -> Value {
        let ratio = |old: &Arm, new: &Arm| {
            let o = old.cpu_ns as f64 / old.checks.max(1) as f64;
            let n = new.cpu_ns as f64 / new.checks.max(1) as f64;
            (n > 0.0).then(|| o / n)
        };
        let f = &self.forward_filter;
        let r = &self.reverse_filter;
        json!({
            "old_forward": self.old_forward.json(), "new_forward": self.new_forward.json(),
            "old_reverse": self.old_reverse.json(), "new_reverse": self.new_reverse.json(),
            "forward_cpu_per_check_ratio_old_over_new": ratio(&self.old_forward, &self.new_forward),
            "reverse_cpu_per_check_ratio_old_over_new": ratio(&self.old_reverse, &self.new_reverse),
            "new_filter": {
                "forward_candidates": f.forward_callbacks,
                "forward_word_rejections": f.forward_bit_rejections,
                "forward_lane_rejections": f.forward_callbacks - f.forward_bit_rejections - f.forward_tests,
                "forward_exact_tests": f.forward_tests,
                "reverse_candidates": r.candidates,
                "reverse_word_rejections": r.words,
                "reverse_lane_rejections": r.candidates - r.words - r.tests,
                "reverse_exact_tests": r.tests,
            },
        })
    }
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
    let metadata: QueueMetadata =
        serde_json::from_value(serde_json::to_value(&sections.queue).unwrap()).unwrap();
    mark("decode", &mut since);
    let mut queue = Queue::<N>::restore_from_parts(
        metadata,
        sections.domains.clone(),
        stored_buckets,
        None,
        &mut Phases::default(),
    )
    .unwrap();
    // The test-only index work counters (atomic adds per group and block)
    // would run in the new arm only; production has none.
    queue.disable_index_work_counters();
    assert!(queue.index_work_counters_disabled_and_zero());
    let queue = queue;
    mark("restore_new_layout", &mut since);
    // CP5: the restored kernel writes the index section byte for byte.
    let index_section_identical = sections.index_round_trips(&queue).unwrap();
    drop(sections);
    mark("index_reencode", &mut since);
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

    // Differential, parallel over requests. Sweeps: every k-th request of
    // each kind (a global stride would alias with the variants' layout).
    let sweep_every = env_usize("RUSTRED_KERNEL_SWEEP", 20).max(1);
    let mut seen = std::collections::BTreeMap::<&str, usize>::new();
    let sweeps: Vec<bool> = requests
        .iter()
        .map(|r| {
            let n = seen.entry(r.kind).or_default();
            *n += 1;
            (*n - 1) % sweep_every == 0
        })
        .collect();
    let per = requests.len().div_ceil(threads).max(1);
    let tallies: Vec<Tally> = std::thread::scope(|scope| {
        let handles: Vec<_> = requests
            .chunks(per)
            .zip(sweeps.chunks(per))
            .map(|(chunk, sweeps)| {
                let (queue, legacy) = (&queue, &legacy);
                scope.spawn(move || {
                    let mut tally = Tally::default();
                    for (request, &sweep) in chunk.iter().zip(sweeps) {
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

    // Timing requests: traced first.
    let timing = env_usize("RUSTRED_KERNEL_TIMING", 20_000).min(requests.len());
    let timed_requests: Vec<&Request> = if traced > 0 {
        requests[..traced].iter().take(timing).collect()
    } else {
        requests.iter().take(timing).collect()
    };
    let mask = allowed_cpus("/proc/self/status");
    let timing_cpu = std::env::var("RUSTRED_KERNEL_TIMING_CPU")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or_else(|| mask.last().copied());
    let arms = Arms {
        queue: &queue,
        legacy: &legacy,
    };

    // (a) One thread pinned to one CPU. It also times the coordinator's
    // `queue_storage` telemetry: the O(blocks) walk that d9163195 ran twice
    // per heartbeat, against the running totals.
    phase("timing_single");
    let (single, single_load, hits, telemetry) = std::thread::scope(|scope| {
        scope
            .spawn(|| {
                let pinned = timing_cpu.filter(|&cpu| pin_thread(cpu));
                let calls = 5;
                let (cpu0, _) = schedstat();
                let started = Instant::now();
                for _ in 0..calls {
                    std::hint::black_box(queue.storage_json());
                }
                let (cpu1, _) = schedstat();
                let totals_wall = started.elapsed().as_nanos() as f64 / calls as f64;
                let totals_cpu = (cpu1 - cpu0) as f64 / calls as f64;
                let started = Instant::now();
                let mut walked = super::super::index::IndexBytes::default();
                for _ in 0..calls {
                    walked = super::super::index::IndexBytes::default();
                    for bucket in queue.by_owner.values() {
                        let b = bucket.indexed.storage_by_walk();
                        walked.blocks += b.blocks;
                        walked.rows += b.rows;
                        walked.live += b.live;
                        walked.lossy += b.lossy;
                    }
                    std::hint::black_box(&walked);
                }
                let (cpu2, _) = schedstat();
                let walk_wall = started.elapsed().as_nanos() as f64 / calls as f64;
                let walk_cpu = (cpu2 - cpu1) as f64 / calls as f64;
                let equal = queue
                    .by_owner
                    .values()
                    .all(|bucket| bucket.indexed.storage() == bucket.indexed.storage_by_walk());
                let telemetry = json!({
                    "calls": calls,
                    "running_totals_storage_json_ms": {"wall": totals_wall * 1e-6, "cpu": totals_cpu * 1e-6},
                    "full_walk_ms_as_in_d9163195": {"wall": walk_wall * 1e-6, "cpu": walk_cpu * 1e-6},
                    "calls_per_coordinator_heartbeat": 2,
                    "walk_totals": {"index_block_bytes": walked.blocks, "index_row_bytes": walked.rows,
                        "live": walked.live, "lossy": walked.lossy},
                    "running_totals_equal_walk": equal,
                    "note": "the walk is measured after the storage_json calls, so its first pass is not colder than a heartbeat's",
                });
                // Unpinned (taskset missing or refused): account the whole mask.
                let window =
                    LoadWindow::begin(&pinned.map_or_else(|| mask.clone(), |cpu| vec![cpu]));
                let mut t = Timing::default();
                for repeat in 0..2 {
                    for (c, chunk) in timed_requests.chunks(250).enumerate() {
                        t.round(&arms, chunk, c, repeat, &|| ());
                    }
                }
                let (own, delay) = t.own();
                let load = window.end(own, delay, pinned);
                // Per-outcome forward costs (hits vs misses), one request at a
                // time right after classifying it: warm, biased towards 1.
                let mut hits = [Arm::default(); 4];
                for r in timed_requests.iter().take(timing / 2) {
                    let hit = legacy.find(r.key, &r.old).0.is_some();
                    let (old, new) = if hit { (0, 1) } else { (2, 3) };
                    timed(&mut hits[old], |a| arms.forward_old(&[*r], a));
                    let mut session = SessionCounters::default();
                    timed(&mut hits[new], |a| arms.forward_new(&[*r], a, &mut session));
                }
                (t, load, hits, telemetry)
            })
            .join()
            .unwrap()
    });

    // (b) Concurrent: one pinned thread per CPU of the mask, every thread on
    // the same arm between barriers, alternating per chunk.
    phase("timing_multi");
    let mt_threads = env_usize("RUSTRED_KERNEL_MT_THREADS", 8).min(mask.len());
    let multi = (mt_threads > 1).then(|| {
        let cpus = &mask[mask.len() - mt_threads..];
        let per_thread = timed_requests.len().div_ceil(mt_threads);
        let rounds = per_thread.div_ceil(250);
        let barrier = std::sync::Barrier::new(mt_threads);
        // Opens the load window once every thread is pinned (the taskset
        // children are outside it), then releases the threads.
        let start = std::sync::Barrier::new(mt_threads + 1);
        let (results, window): (Vec<(Timing, bool)>, LoadWindow) = std::thread::scope(|scope| {
            let handles: Vec<_> = cpus
                .iter()
                .enumerate()
                .map(|(i, &cpu)| {
                    let (arms, barrier, start, timed_requests) =
                        (&arms, &barrier, &start, &timed_requests);
                    scope.spawn(move || {
                        let pinned = pin_thread(cpu);
                        start.wait();
                        start.wait();
                        let mine: Vec<&Request> = timed_requests
                            .iter()
                            .skip(i)
                            .step_by(mt_threads)
                            .copied()
                            .collect();
                        let mut t = Timing::default();
                        let sync = || {
                            barrier.wait();
                        };
                        for repeat in 0..2 {
                            for c in 0..rounds {
                                let chunk = mine.chunks(250).nth(c).unwrap_or(&[]);
                                t.round(arms, chunk, c, repeat, &sync);
                            }
                        }
                        (t, pinned)
                    })
                })
                .collect();
            start.wait();
            let window = LoadWindow::begin(cpus);
            start.wait();
            let results = handles.into_iter().map(|h| h.join().unwrap()).collect();
            (results, window)
        });
        let mut total = Timing::default();
        for (t, _) in &results {
            total.add(t);
        }
        let (own, delay) = total.own();
        let pinned = results.iter().all(|(_, pinned)| *pinned);
        let mut value = total.json();
        value["threads"] = json!(mt_threads);
        value["all_threads_pinned"] = json!(pinned);
        value["load"] = window.end(own, delay, None);
        let inflation = |arm: &str| {
            let per = |v: &Value| v[arm]["cpu_ns_per_check"].as_f64();
            per(&value).zip(per(&single.json())).map(|(m, s)| m / s)
        };
        value["cpu_per_check_multi_over_single"] = json!({
            "old_forward": inflation("old_forward"), "new_forward": inflation("new_forward"),
            "old_reverse": inflation("old_reverse"), "new_reverse": inflation("new_reverse"),
        });
        value
    });
    phase("done");
    mark("timing", &mut since);
    let hit_ratio = |old: &Arm, new: &Arm| {
        let o = old.cpu_ns as f64 / old.checks.max(1) as f64;
        let n = new.cpu_ns as f64 / new.checks.max(1) as f64;
        (n > 0.0).then(|| o / n)
    };
    let exe = std::env::current_exe().ok();
    let exe_blake3 = exe
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| blake3::hash(&bytes).to_hex().to_string());
    let mut single_json = single.json();
    single_json["load"] = single_load.clone();
    let by_kind: serde_json::Map<String, Value> = tally
        .by_kind
        .iter()
        .map(|(kind, k)| (kind.to_string(), k.json()))
        .collect();
    let receipt = json!({
        "provenance": {
            "test_binary": exe,
            "test_binary_blake3": exe_blake3,
            "test_binary_sha256": std::env::var("RUSTRED_KERNEL_BINARY_SHA256").ok(),
            "git_head": std::env::var("RUSTRED_KERNEL_GIT_HEAD").ok(),
            "git_status": std::env::var("RUSTRED_KERNEL_GIT_STATUS").ok(),
            "index_work_counters": "disabled (test-only atomics off in both arms)",
        },
        "checkpoint": dir.to_string_lossy(),
        "arity": N,
        "admitted_ids": ids,
        "live_candidates": queue.containment_candidate_count(),
        "wide_summaries": wide_summaries,
        "image_mismatches": image_mismatches,
        "index_section_byte_identical": index_section_identical,
        "storage": storage,
        "queue_storage_telemetry_cost": telemetry,
        "trace": trace,
        "requests": {"total": requests.len(), "traced": traced,
            "by_kind": tally.by_kind.iter().map(|(k, v)| (k.to_string(), json!(v.requests))).collect::<serde_json::Map<_, _>>()},
        "differential": {
            "forward_pairs": tally.forward_pairs, "reverse_pairs": tally.reverse_pairs,
            "sweep_forward_pairs": tally.sweep_forward_pairs,
            "sweep_reverse_pairs": tally.sweep_reverse_pairs,
            "exact_forward_pairs": tally.exact_forward_pairs,
            "exact_reverse_pairs": tally.exact_reverse_pairs,
            "forward_contained": tally.forward_true, "reverse_contained": tally.reverse_true,
            "mismatches": tally.mismatches, "lookup_mismatches": tally.lookup_mismatches,
            "sweep_selection": format!("every {sweep_every}-th request of each kind"),
            "by_kind": by_kind,
            "examples": tally.examples,
        },
        "timing": {
            "unit": "cpu_ns_per_check = thread on-CPU ns / logical candidates (containment_checks units; reverse: examined candidates)",
            "timed_requests": timed_requests.len(), "repeats": 2, "chunk": 250,
            "single_thread": single_json,
            "multi_thread": multi,
            "old_forward_hit": hits[0].json(), "new_forward_hit": hits[1].json(),
            "old_forward_miss": hits[2].json(), "new_forward_miss": hits[3].json(),
            "hit_ratio": hit_ratio(&hits[0], &hits[1]),
            "miss_ratio": hit_ratio(&hits[2], &hits[3]),
            "hit_miss_caveat": "one request per window right after classifying it (warm); biased towards 1",
        },
        "phases_seconds": phases,
        "total_seconds": started.elapsed().as_secs_f64(),
    });
    std::fs::write(&out_path, serde_json::to_string_pretty(&receipt).unwrap()).unwrap();
    println!("{}", serde_json::to_string_pretty(&receipt).unwrap());
    assert_eq!(image_mismatches, 0);
    assert!(index_section_identical);
    assert_eq!(tally.mismatches, 0, "{:?}", tally.examples);
    assert_eq!(single.old_forward.checks, single.new_forward.checks);
    assert_eq!(single.old_reverse.checks, single.new_reverse.checks);
    assert_eq!(
        receipt["queue_storage_telemetry_cost"]["running_totals_equal_walk"],
        json!(true),
        "running index storage totals differ from the full walk"
    );
}
