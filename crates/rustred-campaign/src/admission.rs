//! Memory and CPU admission: pure, deterministic functions (no I/O, no clock).
//!
//! THE RULE (`ops/MEMORY_RULE.md`, user 2026-11-13, implemented literally, with the coordinator's ruling M2 that
//! reservations are the jobs' CAPS): start a job only if
//!
//! ```text
//!   MemAvailable  -  sum over running jobs of max(cap_i - rss_i, 0)  -  cap_candidate  >=  floor   (default 50 GB)
//! ```
//!
//! MemAvailable already reflects what running jobs USE; `cap_i - rss_i` is what each may still grow (the
//! "reserved-but-not-yet-used memory of started jobs" of the rule); the candidate is charged its cap (= expected
//! peak plus margin >= expected peak, so the literal "MemAvailable - expected peak >= floor" holds a fortiori). Candidates
//! admitted earlier in the same batch count as running jobs with rss 0. GB = 10^9 bytes, as `ops/guarded_run.py`.
//! Further limits: the campaign memory cap (sum of caps), and the core budget (sum of cores).
//!
//! NO STARVATION (decided: aging with a hold): a candidate that waited for a resource while a later candidate was
//! admitted has its `bypassed` counter raised by the caller (the batch result lists the ids). Once `bypassed >=
//! max_bypass`, a waiting candidate HOLDS THE LINE: nothing behind it is admitted until it starts. It holds only if it
//! would fit with none of our jobs running (`fits_when_idle`); a job that cannot fit even then (other efforts use the
//! memory) never blocks the queue, it waits with its reason. Since running jobs end (wall limits), a holder starts
//! after finitely many completions.
//!
//! EMERGENCY (host below the floor anyway, e.g. another effort grew): see [`choose_victims`]. Default order: NEWEST
//! FIRST, stopping as soon as the victims' measured RSS covers the deficit plus a margin. Why: killing frees USED memory
//! only, so the order must rank jobs that hold memory; among those, the newest job has the least work to lose and is the
//! most recent admission bet; old large jobs (the 48-96 GB 14-line tops, hours of work) are spared whenever younger
//! victims suffice. "Largest reserved-over-used first" was rejected: a job far below its cap frees little when killed.
//! Victims are requeued and marked (`host_memory_emergency`); the whole campaign is killed only when the RSS of ALL
//! its jobs does not cover the deficit ("nothing else helps"), and launching pauses while the host is below the floor.
//!
//! MEASUREMENTS WITH TIMESTAMPS (audit A M3, M2): every number carries the monotonic time it was measured at (same
//! clock for all: the caller's, e.g. `Instant` since supervisor start as a `Duration`). A running job's measured memory
//! (`rss_bytes`: PSS where the sampler had it, else RSS; BYTES) reduces its charge below its cap ONLY when the
//! measurement is fresh (`now - rss_measured_at <= max_sample_age`) and not older than the host reading by more than
//! `max_skew`; otherwise the job is charged its FULL cap ("treated as at its cap": all of the cap may still be needed
//! beyond what MemAvailable shows), so a stale or skewed measurement can only make LESS room, never more. A host reading
//! older than `max_sample_age` admits nothing (`StaleMeasurements`). The host number should be the smaller of the two
//! MemAvailable readings bracketing the scan that produced the job numbers (`Snapshot::mem_available_min`): a job that
//! frees memory during the scan is then not counted twice as room. All arithmetic is checked: an overflow is a wait.
//!
//! EMERGENCY persistence and victims in flight (policy point confirmed by the audits): victims are chosen only after
//! `emergency_after_samples` consecutive low readings that also span at least `emergency_persist` (default 5 s: no kill
//! storm for brief foreign pressure); jobs already being killed (`stopping`) stay in `running` and their memory counts
//! as being freed before anything else is chosen; the chosen set is pruned to an inclusion-minimal one.
//!
//! PER-JOB CAP: [`check_cap`]: above its cap a job's group is killed alone (`memory_cap_exceeded`, see
//! `launch::MEMORY_CAP_EXCEEDED`); [`escalate_cap`] gives the next cap for a retry, the scheduler decides whether to
//! retry with it or to move to another ladder rung.

use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const GB: u64 = 1_000_000_000;
pub const DEFAULT_FLOOR_BYTES: u64 = 50 * GB;

/// Victim order of the host-floor emergency.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VictimOrder {
    /// Most recently started first (default).
    NewestFirst,
    /// Largest measured RSS first (fewest kills).
    LargestRssFirst,
    /// Least CPU time spent first (least work lost, by measurement).
    LeastCpuFirst,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AdmissionPolicy {
    /// Host MemAvailable floor (default 50 GB).
    pub floor_bytes: u64,
    /// Limit on the sum of the caps of the campaign's running jobs (None: only the floor).
    pub campaign_mem_cap_bytes: Option<u64>,
    /// Cores the campaign may use at once.
    pub core_budget: usize,
    /// Aging: after this many bypasses a waiting candidate holds the line (default 3).
    pub max_bypass: u32,
    pub victim_order: VictimOrder,
    /// Consecutive below-floor samples before victims are chosen (default 2; launching pauses at the first).
    pub emergency_after_samples: u32,
    /// Free this much more than the deficit (hysteresis; default 2 GB).
    pub emergency_margin_bytes: u64,
    /// A job measurement older than this (against `HostView::now`) is not used: the job is charged its cap (10 s).
    pub max_sample_age: Duration,
    /// A job measurement older than the host reading by more than this is not used (5 s).
    pub max_skew: Duration,
    /// The low readings must span at least this long before victims are chosen (5 s).
    pub emergency_persist: Duration,
}

impl AdmissionPolicy {
    pub fn new(core_budget: usize) -> Self {
        Self {
            floor_bytes: DEFAULT_FLOOR_BYTES,
            campaign_mem_cap_bytes: None,
            core_budget,
            max_bypass: 3,
            victim_order: VictimOrder::NewestFirst,
            emergency_after_samples: 2,
            emergency_margin_bytes: 2 * GB,
            max_sample_age: Duration::from_secs(10),
            max_skew: Duration::from_secs(5),
            emergency_persist: Duration::from_secs(5),
        }
    }
}

/// A job that has been started (from its launch until its final report).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunningJob {
    pub id: u64,
    /// Its memory cap = its reservation.
    pub cap_bytes: u64,
    pub expected_peak_bytes: u64,
    /// Latest measured memory of the job in BYTES (PSS where available, else RSS; 0 before the first sample).
    pub rss_bytes: u64,
    /// When `rss_bytes` was measured (scan start, caller's monotonic clock); None = never: charged its full cap.
    pub rss_measured_at: Option<Duration>,
    pub cores: usize,
    /// Launch order (larger = newer).
    pub started_seq: u64,
    pub cpu_seconds: f64,
    /// Being killed (terminate / kill_now issued, final report not yet in): its memory is being freed.
    pub stopping: bool,
}

impl RunningJob {
    /// Reserved but not yet used, by the raw numbers (no freshness rule; see [`charge`]).
    pub fn outstanding(&self) -> u64 {
        self.cap_bytes.saturating_sub(self.rss_bytes)
    }

    /// Is the measurement usable against this host reading?
    pub fn measurement_fresh(&self, policy: &AdmissionPolicy, host: &HostView) -> bool {
        self.measurement_check(policy, host).is_ok()
    }

    /// Why a measurement is NOT usable (typed input error; every error is treated as stale = the job is charged its full
    /// cap). Gate re-audit B m5: a measurement dated in the FUTURE (after `host.now`) or NEWER than the host reading
    /// (after `host.measured_at`: memory it saw allocated may be missing from the older host number) is rejected.
    pub fn measurement_check(
        &self,
        policy: &AdmissionPolicy,
        host: &HostView,
    ) -> Result<(), MeasurementError> {
        let Some(t) = self.rss_measured_at else {
            return Err(MeasurementError::Missing);
        };
        if t > host.now {
            return Err(MeasurementError::InTheFuture);
        }
        if t > host.measured_at {
            return Err(MeasurementError::NewerThanHost);
        }
        if host.now - t > policy.max_sample_age {
            return Err(MeasurementError::TooOld);
        }
        if t.saturating_add(policy.max_skew) < host.measured_at {
            return Err(MeasurementError::SkewedFromHost);
        }
        Ok(())
    }
}

/// Why a job measurement is not used (it is then treated as stale: the full cap is charged).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum MeasurementError {
    Missing,
    /// Dated after the decision time.
    InTheFuture,
    /// Dated after the host reading it would be combined with.
    NewerThanHost,
    /// Older than `max_sample_age`.
    TooOld,
    /// More than `max_skew` older than the host reading.
    SkewedFromHost,
}

/// What a running job is charged against the floor: `cap - min(rss, cap)` with a fresh measurement, else the cap.
pub fn charge(policy: &AdmissionPolicy, host: &HostView, j: &RunningJob) -> u64 {
    if j.measurement_fresh(policy, host) {
        j.cap_bytes - j.rss_bytes.min(j.cap_bytes)
    } else {
        j.cap_bytes
    }
}

/// A job waiting to start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Candidate {
    pub id: u64,
    pub expected_peak_bytes: u64,
    pub cap_bytes: u64,
    pub cores: usize,
    /// How many times a later candidate was admitted while this one waited (kept by the caller).
    pub bypassed: u32,
}

/// What the host says now.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostView {
    /// `MemInfo::effective_available()` (bytes), ideally the min of the readings bracketing the job measurements.
    pub mem_available_bytes: u64,
    /// When it was read (caller's monotonic clock).
    pub measured_at: Duration,
    /// The decision time (same clock).
    pub now: Duration,
}

impl HostView {
    /// A reading taken now (measured_at = now = 0; jobs then need `rss_measured_at` = Some(0) to count as fresh).
    pub fn at(mem_available_bytes: u64) -> Self {
        Self {
            mem_available_bytes,
            measured_at: Duration::ZERO,
            now: Duration::ZERO,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum WaitReason {
    /// available - outstanding - cap < floor.
    HostFloor {
        available: u64,
        outstanding: u64,
        cap: u64,
        floor: u64,
    },
    CampaignMemCap {
        reserved: u64,
        cap: u64,
        limit: u64,
    },
    CoreBudget {
        in_use: usize,
        need: usize,
        budget: usize,
    },
    /// An older candidate holds the line (aging).
    HeldForOlder {
        holder: u64,
    },
    /// The host reading is older than `max_sample_age`: nothing is admitted on it.
    StaleMeasurements {
        age_ms: u64,
    },
    /// Can never start under this policy (cap above the campaign limit, more cores than the budget).
    NeverFits {
        why: String,
    },
    InvalidCandidate {
        why: String,
    },
}

impl WaitReason {
    /// Waiting for something that running jobs will free (these count for aging).
    pub fn is_resource_wait(&self) -> bool {
        matches!(
            self,
            WaitReason::HostFloor { .. }
                | WaitReason::CampaignMemCap { .. }
                | WaitReason::CoreBudget { .. }
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub enum Decision {
    Admit,
    Wait(WaitReason),
}

/// Sum of the charges (None on overflow).
fn outstanding(policy: &AdmissionPolicy, host: &HostView, running: &[RunningJob]) -> Option<u64> {
    running
        .iter()
        .try_fold(0u64, |a, j| a.checked_add(charge(policy, host, j)))
}

/// Decide one candidate against the running set.
pub fn admit_one(
    policy: &AdmissionPolicy,
    host: HostView,
    running: &[RunningJob],
    c: &Candidate,
) -> Decision {
    if c.cores == 0 {
        return Decision::Wait(WaitReason::InvalidCandidate {
            why: "zero cores".into(),
        });
    }
    let host_age = host.now.saturating_sub(host.measured_at);
    if host_age > policy.max_sample_age {
        return Decision::Wait(WaitReason::StaleMeasurements {
            age_ms: host_age.as_millis().min(u128::from(u64::MAX)) as u64,
        });
    }
    if c.cap_bytes < c.expected_peak_bytes {
        return Decision::Wait(WaitReason::InvalidCandidate {
            why: format!(
                "cap {} below expected peak {}",
                c.cap_bytes, c.expected_peak_bytes
            ),
        });
    }
    if c.cores > policy.core_budget {
        return Decision::Wait(WaitReason::NeverFits {
            why: format!("{} cores > budget {}", c.cores, policy.core_budget),
        });
    }
    if let Some(limit) = policy.campaign_mem_cap_bytes
        && c.cap_bytes > limit
    {
        return Decision::Wait(WaitReason::NeverFits {
            why: format!("cap {} > campaign memory cap {limit}", c.cap_bytes),
        });
    }
    let in_use: usize = running
        .iter()
        .fold(0usize, |a, j| a.saturating_add(j.cores));
    if in_use.saturating_add(c.cores) > policy.core_budget {
        return Decision::Wait(WaitReason::CoreBudget {
            in_use,
            need: c.cores,
            budget: policy.core_budget,
        });
    }
    if let Some(limit) = policy.campaign_mem_cap_bytes {
        let reserved = running
            .iter()
            .try_fold(0u64, |a, j| a.checked_add(j.cap_bytes));
        let total = reserved.and_then(|r| r.checked_add(c.cap_bytes));
        if total.is_none_or(|t| t > limit) {
            let reserved = reserved.unwrap_or(u64::MAX);
            return Decision::Wait(WaitReason::CampaignMemCap {
                reserved,
                cap: c.cap_bytes,
                limit,
            });
        }
    }
    let out = outstanding(policy, &host, running);
    let need = out.and_then(|o| {
        policy
            .floor_bytes
            .checked_add(o)
            .and_then(|x| x.checked_add(c.cap_bytes))
    });
    let out = out.unwrap_or(u64::MAX);
    if need.is_none_or(|n| host.mem_available_bytes < n) {
        return Decision::Wait(WaitReason::HostFloor {
            available: host.mem_available_bytes,
            outstanding: out,
            cap: c.cap_bytes,
            floor: policy.floor_bytes,
        });
    }
    Decision::Admit
}

/// Would `c` fit if none of our jobs were running (their RSS returned to the host)?
pub fn fits_when_idle(
    policy: &AdmissionPolicy,
    host: HostView,
    running: &[RunningJob],
    c: &Candidate,
) -> bool {
    // Only fresh measurements count as memory that would come back.
    let returned = running
        .iter()
        .filter(|j| j.measurement_fresh(policy, &host))
        .map(|j| j.rss_bytes.min(j.cap_bytes))
        .fold(0u64, u64::saturating_add);
    let idle_host = HostView {
        mem_available_bytes: host.mem_available_bytes.saturating_add(returned),
        ..host
    };
    admit_one(policy, idle_host, &[], c) == Decision::Admit
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct BatchOutcome {
    /// One decision per queue entry, in queue order.
    pub decisions: Vec<(u64, Decision)>,
    /// Candidates the caller must mark as bypassed once more (waited for a resource while a later one was admitted).
    pub bypassed: Vec<u64>,
}

impl BatchOutcome {
    pub fn admitted(&self) -> Vec<u64> {
        self.decisions
            .iter()
            .filter(|(_, d)| *d == Decision::Admit)
            .map(|(id, _)| *id)
            .collect()
    }
}

/// Decide a whole queue (in priority order) in one tick.
pub fn decide_batch(
    policy: &AdmissionPolicy,
    host: HostView,
    running: &[RunningJob],
    queue: &[Candidate],
) -> BatchOutcome {
    let mut virt: Vec<RunningJob> = running.to_vec();
    let mut next_seq = running.iter().map(|j| j.started_seq + 1).max().unwrap_or(0);
    let mut waiting: Vec<u64> = Vec::new();
    let mut bypassed: Vec<u64> = Vec::new();
    let mut holder: Option<u64> = None;
    let mut decisions = Vec::with_capacity(queue.len());
    for c in queue {
        if let Some(h) = holder {
            decisions.push((c.id, Decision::Wait(WaitReason::HeldForOlder { holder: h })));
            continue;
        }
        let d = admit_one(policy, host, &virt, c);
        match &d {
            Decision::Admit => {
                for w in waiting.drain(..) {
                    if !bypassed.contains(&w) {
                        bypassed.push(w);
                    }
                }
                virt.push(RunningJob {
                    id: c.id,
                    cap_bytes: c.cap_bytes,
                    expected_peak_bytes: c.expected_peak_bytes,
                    rss_bytes: 0,
                    rss_measured_at: None, // charged its full cap
                    cores: c.cores,
                    started_seq: next_seq,
                    cpu_seconds: 0.0,
                    stopping: false,
                });
                next_seq += 1;
            }
            Decision::Wait(r) if r.is_resource_wait() => {
                waiting.push(c.id);
                if c.bypassed >= policy.max_bypass && fits_when_idle(policy, host, running, c) {
                    holder = Some(c.id);
                }
            }
            Decision::Wait(_) => {}
        }
        decisions.push((c.id, d));
    }
    BatchOutcome {
        decisions,
        bypassed,
    }
}

/// Plan for the host-floor emergency.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EmergencyPlan {
    pub deficit_bytes: u64,
    /// Jobs to kill (their groups only), in kill order; requeue each, marked `host_memory_emergency`.
    pub victims: Vec<u64>,
    /// Measured RSS of the victims.
    pub freed_estimate_bytes: u64,
    /// True when every job holding memory is a victim (nothing less would cover the deficit).
    pub whole_campaign: bool,
    /// Stop launching until the host is back above the floor (always true in a plan).
    pub pause_launching: bool,
}

/// Consecutive low readings, with the time of the first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LowState {
    pub samples: u32,
    /// When the first of these consecutive low readings was taken (caller's monotonic clock).
    pub since: Duration,
}

/// Host below the floor: which jobs to kill. None when the host is at or above the floor. Victims only after
/// `emergency_after_samples` consecutive low readings spanning `emergency_persist`; before that the plan only pauses
/// launching. Jobs with `stopping` (victims in flight) are never chosen again and their memory counts first. The set is
/// pruned to be inclusion-minimal (audit B M4).
pub fn choose_victims(
    policy: &AdmissionPolicy,
    host: HostView,
    running: &[RunningJob],
    low: LowState,
) -> Option<EmergencyPlan> {
    if host.mem_available_bytes >= policy.floor_bytes {
        return None;
    }
    let deficit = policy.floor_bytes - host.mem_available_bytes;
    let mut plan = EmergencyPlan {
        deficit_bytes: deficit,
        victims: vec![],
        freed_estimate_bytes: 0,
        whole_campaign: false,
        pause_launching: true,
    };
    let persisted = host.now.saturating_sub(low.since) >= policy.emergency_persist;
    if low.samples < policy.emergency_after_samples || !persisted {
        return Some(plan);
    }
    let target = deficit.saturating_add(policy.emergency_margin_bytes);
    let in_flight = running
        .iter()
        .filter(|j| j.stopping)
        .map(|j| j.rss_bytes)
        .fold(0u64, u64::saturating_add);
    if in_flight >= target {
        plan.freed_estimate_bytes = in_flight;
        return Some(plan);
    }
    let need = target - in_flight;
    let mut holders: Vec<&RunningJob> = running
        .iter()
        .filter(|j| j.rss_bytes > 0 && !j.stopping)
        .collect();
    match policy.victim_order {
        VictimOrder::NewestFirst => {
            holders.sort_by(|a, b| b.started_seq.cmp(&a.started_seq).then(a.id.cmp(&b.id)))
        }
        VictimOrder::LargestRssFirst => holders.sort_by(|a, b| {
            b.rss_bytes
                .cmp(&a.rss_bytes)
                .then(b.started_seq.cmp(&a.started_seq))
        }),
        VictimOrder::LeastCpuFirst => holders.sort_by(|a, b| {
            a.cpu_seconds
                .total_cmp(&b.cpu_seconds)
                .then(b.started_seq.cmp(&a.started_seq))
        }),
    }
    let mut chosen: Vec<&RunningJob> = Vec::new();
    let mut freed = 0u64;
    for j in &holders {
        if freed >= need {
            break;
        }
        chosen.push(j);
        freed = freed.saturating_add(j.rss_bytes);
    }
    // Prune (in kill order): drop a victim whenever the others still cover the need.
    if freed >= need {
        let mut i = 0;
        while i < chosen.len() {
            let rest = freed - chosen[i].rss_bytes;
            if rest >= need {
                freed = rest;
                chosen.remove(i);
            } else {
                i += 1;
            }
        }
    }
    plan.victims = chosen.iter().map(|j| j.id).collect();
    plan.freed_estimate_bytes = freed.saturating_add(in_flight);
    plan.whole_campaign = !holders.is_empty() && chosen.len() == holders.len();
    Some(plan)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum CapVerdict {
    Within,
    /// Kill this job's group only; outcome `memory_cap_exceeded`.
    Exceeded {
        rss_bytes: u64,
        cap_bytes: u64,
    },
}

pub fn check_cap(job: &RunningJob) -> CapVerdict {
    if job.rss_bytes > job.cap_bytes {
        CapVerdict::Exceeded {
            rss_bytes: job.rss_bytes,
            cap_bytes: job.cap_bytes,
        }
    } else {
        CapVerdict::Within
    }
}

/// Next cap after a `memory_cap_exceeded`: `max(prev * factor, observed peak * factor)`, at most `ceiling`; None when
/// the previous cap was already at the ceiling (the scheduler moves to another rung instead).
pub fn escalate_cap(prev_cap: u64, observed_peak: u64, factor: f64, ceiling: u64) -> Option<u64> {
    if prev_cap >= ceiling {
        return None;
    }
    let f = factor.max(1.0);
    let grown = (prev_cap.max(observed_peak) as f64 * f).ceil();
    let next = if grown.is_finite() && grown < ceiling as f64 {
        grown as u64
    } else {
        ceiling
    };
    Some(next.max(prev_cap.saturating_add(1)).min(ceiling))
}

#[allow(clippy::disallowed_methods)] // test code: the fsx rule (src/fsx.rs) binds production code
#[cfg(test)]
mod tests {
    use super::*;

    /// xorshift64*: deterministic property tests without extra crates.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n.max(1)
        }
    }

    fn cand(id: u64, peak_gb: u64, cap_gb: u64, cores: usize) -> Candidate {
        Candidate {
            id,
            expected_peak_bytes: peak_gb * GB,
            cap_bytes: cap_gb * GB,
            cores,
            bypassed: 0,
        }
    }

    fn job(id: u64, cap_gb: u64, rss_gb: u64, cores: usize, seq: u64) -> RunningJob {
        RunningJob {
            id,
            cap_bytes: cap_gb * GB,
            expected_peak_bytes: cap_gb * GB,
            rss_bytes: rss_gb * GB,
            rss_measured_at: Some(Duration::ZERO),
            cores,
            started_seq: seq,
            cpu_seconds: seq as f64,
            stopping: false,
        }
    }

    #[test]
    fn literal_rule_examples() {
        let p = AdmissionPolicy::new(48);
        let host = HostView::at(100 * GB);
        // 100 - 0 - 50 >= 50: admit exactly at the floor.
        assert_eq!(
            admit_one(&p, host, &[], &cand(1, 40, 50, 1)),
            Decision::Admit
        );
        assert!(matches!(
            admit_one(&p, host, &[], &cand(1, 40, 51, 1)),
            Decision::Wait(WaitReason::HostFloor { .. })
        ));
        // A started job that has not reached its cap still counts: 100 - (30 - 10) - 31 < 50.
        let running = [job(9, 30, 10, 1, 0)];
        assert!(matches!(
            admit_one(&p, host, &running, &cand(1, 20, 31, 1)),
            Decision::Wait(WaitReason::HostFloor { .. })
        ));
        assert_eq!(
            admit_one(&p, host, &running, &cand(1, 20, 30, 1)),
            Decision::Admit
        );
        // Invalid / never.
        assert!(matches!(
            admit_one(&p, host, &[], &cand(1, 40, 30, 1)),
            Decision::Wait(WaitReason::InvalidCandidate { .. })
        ));
        assert!(matches!(
            admit_one(&p, host, &[], &cand(1, 1, 1, 49)),
            Decision::Wait(WaitReason::NeverFits { .. })
        ));
        let mut q = p.clone();
        q.campaign_mem_cap_bytes = Some(30 * GB);
        assert!(matches!(
            admit_one(&q, host, &[], &cand(1, 1, 31, 1)),
            Decision::Wait(WaitReason::NeverFits { .. })
        ));
        assert!(matches!(
            admit_one(&q, host, &running, &cand(1, 1, 1, 1)),
            Decision::Wait(WaitReason::CampaignMemCap { .. })
        ));
        let mut r = p.clone();
        r.core_budget = 2;
        assert!(matches!(
            admit_one(&r, host, &[job(9, 1, 1, 2, 0)], &cand(1, 1, 1, 1)),
            Decision::Wait(WaitReason::CoreBudget { .. })
        ));
    }

    /// PROPERTY: with truthful inputs (every running job stays within its cap, every admitted job within its cap),
    /// the host never falls below the floor because of an admission: worst case after the batch >= floor.
    #[test]
    fn property_never_admits_below_floor() {
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
        let mut admitted_total = 0usize;
        for _ in 0..20_000 {
            let mut p = AdmissionPolicy::new(1 + rng.below(64) as usize);
            p.floor_bytes = rng.below(80) * GB;
            if rng.below(3) == 0 {
                p.campaign_mem_cap_bytes = Some(rng.below(200) * GB);
            }
            let running: Vec<RunningJob> = (0..rng.below(8))
                .map(|i| {
                    let cap = 1 + rng.below(60);
                    job(
                        1000 + i,
                        cap,
                        rng.below(cap + 1),
                        1 + rng.below(4) as usize,
                        i,
                    )
                })
                .collect();
            let queue: Vec<Candidate> = (0..rng.below(10))
                .map(|i| {
                    let peak = rng.below(60);
                    let mut c = cand(i, peak, peak + rng.below(10), 1 + rng.below(8) as usize);
                    c.bypassed = rng.below(5) as u32;
                    c
                })
                .collect();
            let host = HostView::at(rng.below(400) * GB);
            let out = decide_batch(&p, host, &running, &queue);
            let admitted: Vec<&Candidate> = queue
                .iter()
                .filter(|c| out.admitted().contains(&c.id))
                .collect();
            admitted_total += admitted.len();
            if admitted.is_empty() {
                continue;
            }
            let worst = host
                .mem_available_bytes
                .saturating_sub(outstanding(&p, &host, &running).unwrap())
                .saturating_sub(admitted.iter().map(|c| c.cap_bytes).sum());
            assert!(
                worst >= p.floor_bytes,
                "floor broken: {p:?} {host:?} {running:?} {queue:?} -> {out:?}"
            );
            let cores: usize = running.iter().map(|j| j.cores).sum::<usize>()
                + admitted.iter().map(|c| c.cores).sum::<usize>();
            assert!(cores <= p.core_budget);
            if let Some(limit) = p.campaign_mem_cap_bytes {
                let caps: u64 = running.iter().map(|j| j.cap_bytes).sum::<u64>()
                    + admitted.iter().map(|c| c.cap_bytes).sum::<u64>();
                assert!(caps <= limit);
            }
        }
        assert!(
            admitted_total > 1000,
            "the property must be exercised ({admitted_total} admissions)"
        );
    }

    /// Simulation: one large job at the head, an endless stream of small jobs behind it; every job uses its cap at once
    /// (worst case) and runs 3 ticks. Returns the tick at which the large job started (None: never within `ticks`).
    fn simulate(max_bypass: u32, ticks: u64) -> Option<u64> {
        let mut p = AdmissionPolicy::new(64);
        p.max_bypass = max_bypass;
        let external_available = 200 * GB; // what the host has without our jobs
        let mut running: Vec<(RunningJob, u64)> = Vec::new(); // job, end tick
        let mut large = cand(0, 90, 100, 4);
        let mut next_id = 1u64;
        let mut seq = 0u64;
        for t in 0..ticks {
            running.retain(|(_, end)| *end > t);
            let used: u64 = running.iter().map(|(j, _)| j.rss_bytes).sum();
            let host = HostView::at(external_available - used);
            // Queue: the large job (it arrives at tick 5, when the stream already fills the host), then three new
            // small jobs each tick (a stream that alone keeps the host full).
            let mut queue = if t >= 5 { vec![large.clone()] } else { vec![] };
            for _ in 0..3 {
                queue.push(cand(next_id, 9, 10, 1));
                next_id += 1;
            }
            let rj: Vec<RunningJob> = running.iter().map(|(j, _)| j.clone()).collect();
            let out = decide_batch(&p, host, &rj, &queue);
            if out.bypassed.contains(&0) {
                large.bypassed += 1;
            }
            for id in out.admitted() {
                if id == 0 {
                    return Some(t);
                }
                let c = queue.iter().find(|c| c.id == id).unwrap();
                running.push((
                    RunningJob {
                        id,
                        cap_bytes: c.cap_bytes,
                        expected_peak_bytes: c.expected_peak_bytes,
                        rss_bytes: c.cap_bytes,
                        rss_measured_at: Some(Duration::ZERO),
                        cores: c.cores,
                        started_seq: seq,
                        cpu_seconds: 0.0,
                        stopping: false,
                    },
                    t + 3,
                ));
                seq += 1;
            }
        }
        None
    }

    /// PROPERTY: no starvation of a large job by a stream of small ones (aging with a hold), and the negative control:
    /// without aging the same stream starves it.
    #[test]
    fn property_no_starvation_with_aging() {
        let started = simulate(3, 1_000).expect("the large job must start");
        assert!(
            (5..=25).contains(&started),
            "large job started at tick {started}"
        );
        assert_eq!(
            simulate(u32::MAX, 1_000),
            None,
            "control: without aging the stream starves the large job"
        );
    }

    #[test]
    fn a_job_that_cannot_fit_even_when_idle_does_not_block() {
        let mut p = AdmissionPolicy::new(8);
        p.max_bypass = 0;
        let host = HostView::at(80 * GB);
        let mut big = cand(0, 100, 100, 1);
        big.bypassed = 10;
        let out = decide_batch(&p, host, &[], &[big, cand(1, 5, 5, 1)]);
        assert_eq!(out.admitted(), vec![1]);
        assert_eq!(out.bypassed, vec![0]);
    }

    #[test]
    fn hold_blocks_later_candidates() {
        let mut p = AdmissionPolicy::new(8);
        p.max_bypass = 2;
        // MemAvailable 80 already excludes the running job's 30 GB: 30 GB above the floor; idle it would be 60.
        let host = HostView::at(80 * GB);
        let running = [job(9, 30, 30, 1, 0)];
        let mut big = cand(0, 40, 40, 1);
        big.bypassed = 2;
        let out = decide_batch(&p, host, &running, &[big.clone(), cand(1, 5, 5, 1)]);
        assert!(out.admitted().is_empty());
        assert_eq!(
            out.decisions[1].1,
            Decision::Wait(WaitReason::HeldForOlder { holder: 0 })
        );
        big.bypassed = 1;
        let out = decide_batch(&p, host, &running, &[big, cand(1, 5, 5, 1)]);
        assert_eq!(out.admitted(), vec![1]);
        assert_eq!(out.bypassed, vec![0]);
    }

    /// Host view at t = 10 s; low readings since t = 0 (persisted) unless stated.
    fn hv(gb: u64) -> HostView {
        HostView {
            mem_available_bytes: gb * GB,
            measured_at: Duration::from_secs(10),
            now: Duration::from_secs(10),
        }
    }

    fn low(n: u32) -> LowState {
        LowState {
            samples: n,
            since: Duration::ZERO,
        }
    }

    #[test]
    fn emergency_victims() {
        let p = AdmissionPolicy::new(64);
        let running = [
            job(1, 40, 30, 1, 0),
            job(2, 10, 8, 1, 1),
            job(3, 10, 6, 1, 2),
            job(4, 5, 0, 1, 3),
        ];
        assert_eq!(choose_victims(&p, hv(50), &running, low(5)), None);
        // Deficit 4 GB + 2 GB margin: newest holder (3: 6 GB) suffices; job 4 holds nothing and is skipped.
        let first = choose_victims(&p, hv(46), &running, low(1)).unwrap();
        assert!(first.victims.is_empty() && first.pause_launching);
        // Two samples, but within 5 s: not persisted yet.
        let brief = choose_victims(
            &p,
            hv(46),
            &running,
            LowState {
                samples: 2,
                since: Duration::from_secs(8),
            },
        )
        .unwrap();
        assert!(brief.victims.is_empty());
        let plan = choose_victims(&p, hv(46), &running, low(2)).unwrap();
        assert_eq!(plan.victims, vec![3]);
        assert!(!plan.whole_campaign);
        // Deficit 12 + 2: 3 (6) + 2 (8) = 14 suffices; the old 30 GB job is spared.
        let plan = choose_victims(&p, hv(38), &running, low(2)).unwrap();
        assert_eq!(plan.victims, vec![3, 2]);
        // Largest first: one kill.
        let mut q = p.clone();
        q.victim_order = VictimOrder::LargestRssFirst;
        assert_eq!(
            choose_victims(&q, hv(38), &running, low(2))
                .unwrap()
                .victims,
            vec![1]
        );
        // Deficit beyond everything we hold: all holders, whole_campaign.
        let plan = choose_victims(&p, hv(0), &running, low(2)).unwrap();
        assert_eq!(plan.victims, vec![3, 2, 1]);
        assert!(plan.whole_campaign);
        // A victim already being killed counts as freed: nothing more is chosen while it covers the need.
        let mut inflight = running.clone();
        inflight[2].stopping = true;
        let plan = choose_victims(&p, hv(46), &inflight, low(3)).unwrap();
        assert!(plan.victims.is_empty(), "{plan:?}");
    }

    #[test]
    fn cap_check_and_escalation() {
        assert_eq!(check_cap(&job(1, 10, 10, 1, 0)), CapVerdict::Within);
        assert!(matches!(
            check_cap(&job(1, 10, 11, 1, 0)),
            CapVerdict::Exceeded { .. }
        ));
        assert_eq!(escalate_cap(10 * GB, 12 * GB, 1.5, 100 * GB), Some(18 * GB));
        assert_eq!(escalate_cap(80 * GB, 0, 1.5, 100 * GB), Some(100 * GB));
        assert_eq!(escalate_cap(100 * GB, 0, 1.5, 100 * GB), None);
    }
}
