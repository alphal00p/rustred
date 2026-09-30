//! Opt-in process-local scheduling observations. Never read by admission,
//! publication or checkpoint authority. The default path allocates nothing.
use super::super::super::inspection::{Effect, Event};
use super::super::job::Job;
use super::{Activity, Status};
use serde::Serialize;
use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;
use std::time::Instant;

const RETAIN: usize = 32;
pub(in super::super) const ENVIRONMENT: &str = "RUSTRED_EPOCH_PROFILE";

pub(in super::super) struct Collector {
    origin: Instant,
    jobs: Mutex<Jobs>,
}

impl Default for Collector {
    fn default() -> Self {
        Self {
            origin: Instant::now(),
            jobs: Mutex::new(Jobs::default()),
        }
    }
}

impl Collector {
    pub fn from_environment() -> Option<Self> {
        (std::env::var_os(ENVIRONMENT).as_deref() == Some(std::ffi::OsStr::new("1")))
            .then(Self::default)
    }

    pub fn snapshot(&self) -> Jobs {
        // A diagnostic failure must not turn valid mathematical work into an
        // engine failure. Poisoning is explicitly reported, not hidden.
        match self.jobs.lock() {
            Ok(jobs) => jobs.clone(),
            Err(poisoned) => {
                let mut jobs = poisoned.into_inner().clone();
                jobs.poisoned = true;
                jobs
            }
        }
    }

    pub fn origin(&self) -> Instant {
        self.origin
    }

    pub fn record<const N: usize>(&self, job: &Job<N>, sample: Observation, failure: Outcome) {
        let seconds = sample.started.elapsed().as_secs_f64();
        let Ok(mut jobs) = self.jobs.lock() else {
            return;
        };
        jobs.jobs += 1;
        jobs.events += sample.events;
        jobs.admits += sample.admits;
        jobs.seconds += seconds;
        jobs.eventless += u64::from(sample.first_event.is_none());
        jobs.admitless += u64::from(sample.first_admit.is_none());
        jobs.pre_event_seconds += sample.first_event.unwrap_or(seconds);
        jobs.pre_admit_seconds += sample.first_admit.unwrap_or(seconds);
        jobs.visitor_seconds += sample.visitor_seconds.unwrap_or(seconds);
        jobs.errors += u64::from(failure.error);
        jobs.panics += u64::from(failure.panic);
        jobs.cancel_requested += u64::from(failure.cancel_requested);
        if jobs.slowest[RETAIN - 1].is_none_or(|last| {
            seconds > last.seconds || (seconds == last.seconds && job.seq < last.seq)
        }) {
            let record = JobSample {
                seq: job.seq,
                parent: job.parent,
                phase: match job.image.phase() {
                    super::super::super::queue::Phase::Apply => "Apply",
                    super::super::super::queue::Phase::Route => "Route",
                },
                owner_bits: job
                    .image
                    .owner()
                    .iter()
                    .enumerate()
                    .fold(0, |bits, (axis, &set)| bits | (u32::from(set) << axis)),
                arity: N,
                started_seconds: sample
                    .started
                    .saturating_duration_since(self.origin)
                    .as_secs_f64(),
                seconds,
                first_event_seconds: sample.first_event,
                first_admit_seconds: sample.first_admit,
                visitor_seconds: sample.visitor_seconds,
                events: sample.events,
                admits: sample.admits,
                outcome: failure,
            };
            let at = jobs.slowest.partition_point(|old| {
                old.is_some_and(|old| {
                    old.seconds > seconds || (old.seconds == seconds && old.seq <= job.seq)
                })
            });
            jobs.slowest[at..].rotate_right(1);
            jobs.slowest[at] = Some(record);
        }
    }
}

#[derive(Clone, Serialize)]
pub(in super::super) struct Jobs {
    enabled: bool,
    version: u32,
    scope: &'static str,
    pub poisoned: bool,
    jobs: u64,
    events: u64,
    admits: u64,
    eventless: u64,
    admitless: u64,
    errors: u64,
    panics: u64,
    cancel_requested: u64,
    seconds: f64,
    pre_event_seconds: f64,
    pre_admit_seconds: f64,
    visitor_seconds: f64,
    slowest: [Option<JobSample>; RETAIN],
}

impl Default for Jobs {
    fn default() -> Self {
        Self {
            enabled: true,
            version: 1,
            scope: "invocation-local visitor-started jobs; excludes malformed input/snapshot rejection before visitor; includes later rejected/cancelled results; summed worker wall, not CPU/critical path; started_seconds shares monotonic origin with wait samples; other offsets job-relative; first Admit may be redundant; missing first-event/Admit charges whole job; never authority",
            poisoned: false,
            jobs: 0,
            events: 0,
            admits: 0,
            eventless: 0,
            admitless: 0,
            errors: 0,
            panics: 0,
            cancel_requested: 0,
            seconds: 0.0,
            pre_event_seconds: 0.0,
            pre_admit_seconds: 0.0,
            visitor_seconds: 0.0,
            slowest: [None; RETAIN],
        }
    }
}

#[derive(Clone, Copy, Default, Serialize)]
pub(in super::super) struct Outcome {
    pub error: bool,
    pub panic: bool,
    pub cancel_requested: bool,
}

#[derive(Clone, Copy, Serialize)]
struct JobSample {
    seq: u64,
    parent: u32,
    phase: &'static str,
    owner_bits: u32,
    arity: usize,
    started_seconds: f64,
    seconds: f64,
    first_event_seconds: Option<f64>,
    first_admit_seconds: Option<f64>,
    visitor_seconds: Option<f64>,
    events: u64,
    admits: u64,
    outcome: Outcome,
}

pub(in super::super) struct Observation {
    started: Instant,
    first_event: Option<f64>,
    first_admit: Option<f64>,
    visitor_seconds: Option<f64>,
    events: u64,
    admits: u64,
}

impl Observation {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            first_event: None,
            first_admit: None,
            visitor_seconds: None,
            events: 0,
            admits: 0,
        }
    }
    pub fn event<const N: usize>(&mut self, event: &Event<N>) {
        self.events += event.count as u64;
        if self.first_event.is_none() {
            self.first_event = Some(self.started.elapsed().as_secs_f64());
        }
        if matches!(&event.effect, Effect::Admit { .. }) {
            self.admits += event.count as u64;
            if self.first_admit.is_none() {
                self.first_admit = Some(self.started.elapsed().as_secs_f64());
            }
        }
    }
    pub fn visitor_finished(&mut self) {
        self.visitor_seconds = Some(self.started.elapsed().as_secs_f64());
    }
}

#[derive(Clone, Copy, Default, Serialize)]
struct Bucket {
    calls: u64,
    seconds: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum WaitKind {
    PublicationRetention,
    NoIssuedWork,
    InlineInspection,
    PrefixQueued,
    PrefixCreditBlocked,
    PrefixComputing,
    ReceiptDrain,
    PrefixReady,
    Unknown,
}

impl WaitKind {
    const COUNT: usize = 9;
}

/// One poll interval, classified by its start observation. Worker state may
/// change during the poll: weighted values are estimates, not exact CPU time.
#[derive(Clone, Copy, Serialize)]
pub(in super::super) struct WaitSample {
    kind: WaitKind,
    earliest_missing_prefix: Option<u64>,
    pending: usize,
    queued: usize,
    computing: usize,
    returned: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    logical_issued: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    logical_returned: Option<usize>,
    activity_available: bool,
    inspector_capacity: usize,
    prefix_started: Option<bool>,
    prefix_returned: Option<bool>,
    polls: u64,
    mixed_kinds: bool,
    started_seconds: f64,
    ended_seconds: f64,
    seconds: f64,
}

impl WaitSample {
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        publication: bool,
        issued: bool,
        missing: Option<u64>,
        window: usize,
        inspector_capacity: usize,
        pending: usize,
        activity: Option<(Activity, Option<Status>)>,
    ) -> Self {
        Self::capture_with_inventory(
            publication,
            issued,
            missing,
            window,
            inspector_capacity,
            pending,
            activity,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn capture_with_inventory(
        publication: bool,
        issued: bool,
        missing: Option<u64>,
        window: usize,
        inspector_capacity: usize,
        pending: usize,
        activity: Option<(Activity, Option<Status>)>,
        logical: Option<(usize, usize)>,
    ) -> Self {
        let (a, selected) = activity.unwrap_or_default();
        let (occupied, returned) = logical.unwrap_or((a.occupied, a.returned));
        let kind = if publication {
            WaitKind::PublicationRetention
        } else if !issued {
            WaitKind::NoIssuedWork
        } else if a.inline {
            WaitKind::InlineInspection
        } else if missing.is_none() {
            WaitKind::PrefixReady
        } else if let Some(selected) = selected {
            if selected.returned {
                WaitKind::ReceiptDrain
            } else if !selected.started {
                WaitKind::PrefixQueued
            } else if occupied >= window
                && returned != 0
                && pending != 0
                && a.queued == 0
                && a.computing < inspector_capacity
            {
                WaitKind::PrefixCreditBlocked
            } else {
                WaitKind::PrefixComputing
            }
        } else {
            WaitKind::Unknown
        };
        Self {
            kind,
            earliest_missing_prefix: missing,
            pending,
            queued: a.queued,
            computing: a.computing,
            returned: a.returned,
            logical_issued: logical.map(|x| x.0),
            logical_returned: logical.map(|x| x.1),
            activity_available: activity.is_some(),
            inspector_capacity,
            prefix_started: selected.map(|s| s.started),
            prefix_returned: selected.map(|s| s.returned),
            polls: 1,
            mixed_kinds: false,
            started_seconds: 0.0,
            ended_seconds: 0.0,
            seconds: 0.0,
        }
    }
}

pub(in super::super) fn earliest_missing<T>(
    order: &VecDeque<u64>,
    results: &BTreeMap<u64, T>,
    cut: usize,
) -> Option<u64> {
    order
        .iter()
        .take(cut)
        .find(|key| !results.contains_key(key))
        .copied()
}

#[derive(Serialize)]
pub(in super::super) struct Waits {
    #[serde(skip)]
    origin: Instant,
    enabled: bool,
    version: u32,
    scope: &'static str,
    kinds: [&'static str; WaitKind::COUNT],
    buckets: [Bucket; WaitKind::COUNT],
    calls: u64,
    seconds: f64,
    queued_worker_seconds: f64,
    computing_worker_seconds: f64,
    returned_result_seconds: f64,
    pending_work_wait_seconds: f64,
    missing_activity_seconds: f64,
    current_blocker: Option<WaitSample>,
    longest_blocker_windows: [Option<WaitSample>; RETAIN],
}

impl Default for Waits {
    fn default() -> Self {
        Self::new(Instant::now())
    }
}

impl Waits {
    pub fn new(origin: Instant) -> Self {
        Self {
            origin,
            enabled: true,
            version: 1,
            scope: "invocation-local blocking polls; exclusive start-state classes sum to blocking_poll_seconds; occupancy times are start-sampled wall estimates, not CPU; common monotonic origin with job starts; consecutive same-prefix polls coalesced, seconds sums blocked wall not interval span; retained windows32 plus current; sample activity/kind are first state (mixed_kinds flags changes); no authority",
            kinds: [
                "publication_retention",
                "no_issued_work",
                "inline_inspection",
                "prefix_queued",
                "prefix_credit_blocked",
                "prefix_computing",
                "receipt_drain",
                "prefix_ready",
                "unknown",
            ],
            buckets: [Bucket::default(); WaitKind::COUNT],
            calls: 0,
            seconds: 0.0,
            queued_worker_seconds: 0.0,
            computing_worker_seconds: 0.0,
            returned_result_seconds: 0.0,
            pending_work_wait_seconds: 0.0,
            missing_activity_seconds: 0.0,
            current_blocker: None,
            longest_blocker_windows: [None; RETAIN],
        }
    }
}

impl Waits {
    pub fn record(&mut self, mut sample: WaitSample, started: Instant, seconds: f64) {
        sample.started_seconds = started.saturating_duration_since(self.origin).as_secs_f64();
        sample.ended_seconds = sample.started_seconds + seconds;
        sample.seconds = seconds;
        self.calls += 1;
        self.seconds += seconds;
        self.buckets[sample.kind as usize].calls += 1;
        self.buckets[sample.kind as usize].seconds += seconds;
        self.queued_worker_seconds += sample.queued as f64 * seconds;
        self.computing_worker_seconds += sample.computing as f64 * seconds;
        self.returned_result_seconds += sample.returned as f64 * seconds;
        if sample.pending != 0 {
            self.pending_work_wait_seconds += seconds;
        }
        if !sample.activity_available {
            self.missing_activity_seconds += seconds;
        }
        if let Some(current) = &mut self.current_blocker
            && sample.earliest_missing_prefix.is_some()
            && current.earliest_missing_prefix == sample.earliest_missing_prefix
        {
            current.polls += 1;
            current.seconds += seconds;
            current.ended_seconds = sample.ended_seconds;
            current.mixed_kinds |= current.kind != sample.kind;
        } else {
            self.finish();
            self.current_blocker = Some(sample);
        }
    }

    pub fn finish(&mut self) {
        let Some(sample) = self.current_blocker.take() else {
            return;
        };
        if self.longest_blocker_windows[RETAIN - 1].is_none_or(|last| sample.seconds > last.seconds)
        {
            let at = self
                .longest_blocker_windows
                .partition_point(|old| old.is_some_and(|old| old.seconds >= sample.seconds));
            self.longest_blocker_windows[at..].rotate_right(1);
            self.longest_blocker_windows[at] = Some(sample);
        }
    }
}

#[cfg(test)]
mod tests;
