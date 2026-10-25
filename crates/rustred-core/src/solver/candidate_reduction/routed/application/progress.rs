//! Optional caller-thread observations of exact application, never CAS work.
use std::time::{Duration, Instant};

use super::CandidateRoutedApplicationStatistics;
use crate::family::IntegralKey;
use crate::reduction::CacheWeight;

/// Charged completed coefficient payload, including guards. These counts do
/// not include pending frames, output scratch, allocator overhead or process RSS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedCacheCensus {
    pub entries: usize,
    pub coefficient_terms: usize,
    pub coefficient_bytes: usize,
}

impl CandidateRoutedCacheCensus {
    pub(super) fn new(entries: usize, weight: CacheWeight) -> Self {
        Self {
            entries,
            coefficient_terms: weight.coefficient_terms,
            coefficient_bytes: weight.coefficient_bytes,
        }
    }

    pub(super) fn retain(&mut self, weight: CacheWeight) {
        // A subset of the already admitted cache census cannot overflow. The
        // caller updates this only after budget admission or first pinning.
        self.entries += 1;
        self.coefficient_terms += weight.coefficient_terms;
        self.coefficient_bytes += weight.coefficient_bytes;
    }

    pub(super) fn release(&mut self, weight: CacheWeight) {
        self.entries -= 1;
        self.coefficient_terms -= weight.coefficient_terms;
        self.coefficient_bytes -= weight.coefficient_bytes;
    }

    pub(super) fn excluding(self, other: Self) -> Self {
        Self {
            entries: self.entries - other.entries,
            coefficient_terms: self.coefficient_terms - other.coefficient_terms,
            coefficient_bytes: self.coefficient_bytes - other.coefficient_bytes,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedApplicationPhase {
    Admission,
    Expansion,
    Combination,
    RootCollection,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedApplicationStatus {
    Running,
    Succeeded,
    Failed,
}

/// One coherent safe-boundary observation. Work is local to this request;
/// retained cache counts also include reusable entries from earlier requests.
/// A successful core request precedes any caller-owned terminal substitutions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedApplicationSnapshot {
    pub sequence: u64,
    pub elapsed: Duration,
    pub phase: CandidateRoutedApplicationPhase,
    pub status: CandidateRoutedApplicationStatus,
    /// Request counters, with `operational_nodes` updated in this snapshot copy
    /// to match the current number of distinct entered nodes.
    pub statistics: CandidateRoutedApplicationStatistics,
    pub unique_nodes: usize,
    pub active_nodes: usize,
    pub pending_frames: usize,
    pub cached: CandidateRoutedCacheCensus,
    /// Evicting policies maintain dependency pins. None under FailOnLimit, not zero.
    pub pinned: Option<CandidateRoutedCacheCensus>,
    pub eligible: Option<CandidateRoutedCacheCensus>,
}

/// Runtime observation costs only. Exhausting these sampling grants omits detail;
/// it never restricts reduction work, changes admission, or supplies an ETA bound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedObservationOptions {
    pub interval: Duration,
    pub max_frontier_frames: usize,
    pub max_path_nodes: usize,
}
impl Default for CandidateRoutedObservationOptions {
    fn default() -> Self {
        Self {
            interval: Duration::from_millis(250),
            max_frontier_frames: 256,
            max_path_nodes: 64,
        }
    }
}

/// Borrowed view of the existing Route/Apply identity. Only physical slots are
/// exposed; equal integrals in different operational stages remain distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CandidateRoutedWorkRef<'a> {
    Route(&'a IntegralKey),
    Apply {
        owner_sector: &'a [bool],
        target: &'a IntegralKey,
    },
}
impl CandidateRoutedWorkRef<'_> {
    pub fn target(&self) -> &IntegralKey {
        match self {
            Self::Route(key) | Self::Apply { target: key, .. } => key,
        }
    }
}

/// Canonical family-key measures, not physical input-expression ranks or a
/// globally descending scalar. D is A-R; dot excess is A-support.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedKeyDegrees {
    pub numerator_rank: u128,
    pub positive_power_sum: u128,
    pub power_difference: i128,
    pub support: usize,
    pub dot_excess: u128,
}
impl CandidateRoutedKeyDegrees {
    pub fn from_key(key: &IntegralKey) -> Self {
        let mut value = Self::default();
        for &power in key.powers() {
            value.power_difference = value.power_difference.saturating_add(i128::from(power));
            if power < 0 {
                value.numerator_rank = value
                    .numerator_rank
                    .saturating_add(u128::from(power.unsigned_abs()));
            }
            if power > 0 {
                value.support += 1;
                value.positive_power_sum = value.positive_power_sum.saturating_add(power as u128);
                value.dot_excess = value
                    .dot_excess
                    .saturating_add((power as u128).saturating_sub(1));
            }
        }
        value
    }
    pub(super) fn include(&mut self, other: Self) {
        self.numerator_rank = self.numerator_rank.max(other.numerator_rank);
        self.positive_power_sum = self.positive_power_sum.max(other.positive_power_sum);
        self.power_difference = self.power_difference.max(other.power_difference);
        self.support = self.support.max(other.support);
        self.dot_excess = self.dot_excess.max(other.dot_excess);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CandidateRoutedActivity {
    #[default]
    Admission,
    Expansion,
    OwnerPreparation,
    Transport,
    RuleEvaluation,
    CoefficientCombination,
    RootMaterialization,
    RootCollection,
    TerminalSubstitution,
    /// A caller-owned operation, such as numerical master evaluation. Native
    /// observations do not imply completion of this enclosing operation.
    Caller,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedCompletionSource {
    Computed,
    RamHit,
    BackingHit,
    Zero,
    Terminal,
    UnresolvedPreserved,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedApplicationEvent {
    Snapshot,
    ActivityStarted,
    NodeCompleted(CandidateRoutedCompletionSource),
    RootCollected,
    CycleDetected,
    CoreFinished,
    TerminalSubstitutionStarted,
    TerminalSubstitutionProgress,
    RequestFinished,
    CallerProgress,
}

/// Per native request. Ready node sums can contain unresolved keys in Preserve
/// mode; neither completion nor local descent establishes family closure.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedApplicationMetrics {
    pub completed_unique_operational_nodes: usize,
    pub completion_attempts: usize,
    pub completed_roots: usize,
    pub requested_roots: Option<usize>,
    pub output_terms: Option<usize>,
    /// Surviving rule-child edges after the evaluator checked persisted order;
    /// not a count of CAS comparisons, nor a global remaining-work certificate.
    pub observed_descent_checks: usize,
    pub max_seen_degrees: CandidateRoutedKeyDegrees,
}

/// Distinct queued Enter identities and physical integrals in a bounded frame
/// sample. Counts are lower bounds unless exact; future discoveries are unknown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedFrontierSample {
    pub frames_examined: usize,
    pub total_pending_frames: usize,
    pub unique_nodes: usize,
    pub unique_integrals: usize,
    pub exact: bool,
}

/// Existing structure sizes/censuses only. Bytes describe retained native
/// payload, including guards, not encoded bytes or scratch/RSS. Factorized
/// coefficients are never materialized solely to obtain observation detail.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedComplexity {
    pub terms: Option<usize>,
    pub coefficient_terms: Option<usize>,
    pub coefficient_bytes: Option<usize>,
    pub factorized: bool,
    /// One-based index of the current native root-output contribution.
    pub contribution_index: Option<usize>,
    pub contribution_total: Option<usize>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CandidateRoutedTerminalProgress {
    pub rows: usize,
    pub completed_rows: usize,
    pub additions: usize,
    /// Caller-visible substituted output map size, separate from core output.
    pub output_terms: Option<usize>,
}

/// One immutable callback-lifetime view. It owns no coefficient/Atom payload.
/// Copy only bounded metadata before returning; callbacks must not reenter the
/// engine (a caller may hold an application mutex) or block on a renderer.
#[derive(Clone, Copy, Debug)]
pub struct CandidateRoutedApplicationEnvelope<'a> {
    pub sequence: u64,
    pub elapsed: Duration,
    pub snapshot: Option<CandidateRoutedApplicationSnapshot>,
    /// Age of the latest genuine native boundary. Forwarding its summary into
    /// saved/caller stages does not make the native work current again.
    pub native_summary_age: Option<Duration>,
    pub event: CandidateRoutedApplicationEvent,
    pub activity: CandidateRoutedActivity,
    pub status: CandidateRoutedApplicationStatus,
    pub request_finished: bool,
    pub family_fingerprint: &'a str,
    pub root: Option<CandidateRoutedWorkRef<'a>>,
    pub current: Option<CandidateRoutedWorkRef<'a>>,
    pub degrees: Option<CandidateRoutedKeyDegrees>,
    pub metrics: CandidateRoutedApplicationMetrics,
    pub frontier: Option<CandidateRoutedFrontierSample>,
    pub active_path: &'a [CandidateRoutedWorkRef<'a>],
    pub path_total_nodes: usize,
    /// Index in the full ancestor path; it may lie beyond the retained prefix.
    pub cycle_backedge: Option<usize>,
    pub complexity: CandidateRoutedComplexity,
    pub terminal: Option<CandidateRoutedTerminalProgress>,
    /// Only caller-owned stages use this concise static label.
    pub caller_phase: Option<&'a str>,
}
impl<'a> CandidateRoutedApplicationEnvelope<'a> {
    pub fn new(event: CandidateRoutedApplicationEvent, family_fingerprint: &'a str) -> Self {
        Self {
            sequence: 0,
            elapsed: Duration::ZERO,
            snapshot: None,
            native_summary_age: None,
            event,
            activity: CandidateRoutedActivity::Admission,
            status: CandidateRoutedApplicationStatus::Running,
            request_finished: false,
            family_fingerprint,
            root: None,
            current: None,
            degrees: None,
            metrics: CandidateRoutedApplicationMetrics::default(),
            frontier: None,
            active_path: &[],
            path_total_nodes: 0,
            cycle_backedge: None,
            complexity: CandidateRoutedComplexity::default(),
            terminal: None,
            caller_phase: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedObserverControl {
    Continue,
    Detach,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedObserverFault {
    CallbackError,
    Panicked,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandidateRoutedObserverStatus {
    Active,
    Detached,
    Faulted(CandidateRoutedObserverFault),
}

type ObservationCallback<'a> = dyn FnMut(&CandidateRoutedApplicationEnvelope<'_>) -> Result<CandidateRoutedObserverControl, ()>
    + 'a;

/// Optional per-call telemetry. Faults/detachment disable delivery without
/// replacing the reduction's typed result. Legacy snapshot APIs retain their
/// documented panic/unwind and cancellation contracts. Panic isolation requires
/// unwind support; an aborting process cannot be recovered by this API.
pub struct CandidateRoutedApplicationObserver<'a> {
    options: CandidateRoutedObservationOptions,
    callback: &'a mut ObservationCallback<'a>,
    status: CandidateRoutedObserverStatus,
    started: Instant,
    sequence: u64,
    last_snapshot: Option<CandidateRoutedApplicationSnapshot>,
    last_metrics: CandidateRoutedApplicationMetrics,
    last_native_observation: Option<Duration>,
}
impl<'a> CandidateRoutedApplicationObserver<'a> {
    pub fn new(
        options: CandidateRoutedObservationOptions,
        callback: &'a mut ObservationCallback<'a>,
    ) -> Self {
        Self {
            options,
            callback,
            status: CandidateRoutedObserverStatus::Active,
            started: Instant::now(),
            sequence: 0,
            last_snapshot: None,
            last_metrics: CandidateRoutedApplicationMetrics::default(),
            last_native_observation: None,
        }
    }
    pub fn options(&self) -> CandidateRoutedObservationOptions {
        self.options
    }
    pub fn status(&self) -> CandidateRoutedObserverStatus {
        self.status
    }
    pub fn last_snapshot(&self) -> Option<CandidateRoutedApplicationSnapshot> {
        self.last_snapshot
    }
    pub fn last_metrics(&self) -> CandidateRoutedApplicationMetrics {
        self.last_metrics
    }
    /// Begin a saved/caller request before native preflight. This prevents an
    /// early admission/binding error from borrowing a previous request's core
    /// summary, while retaining this observer's sequence and detach/fault state.
    pub fn clear_native_summary(&mut self) {
        self.last_snapshot = None;
        self.last_metrics = CandidateRoutedApplicationMetrics::default();
        self.last_native_observation = None;
    }
    /// Emit an enclosing caller/application boundary through the same observer.
    /// This does no reduction or error conversion. Native snapshots keep their
    /// own scope; callers must label their outer phase instead of rewriting it.
    pub fn emit(&mut self, mut envelope: CandidateRoutedApplicationEnvelope<'_>) {
        if self.status != CandidateRoutedObserverStatus::Active {
            return;
        }
        let elapsed = self.started.elapsed();
        if let Some(snapshot) = envelope.snapshot {
            self.last_snapshot = Some(snapshot);
            self.last_metrics = envelope.metrics;
            if envelope.caller_phase.is_none()
                && envelope.activity != CandidateRoutedActivity::Caller
            {
                self.last_native_observation = Some(elapsed);
            }
        }
        self.sequence = self.sequence.saturating_add(1);
        envelope.sequence = self.sequence;
        envelope.elapsed = elapsed;
        envelope.native_summary_age = self
            .last_native_observation
            .filter(|_| envelope.snapshot.is_some())
            .map(|then| elapsed.saturating_sub(then));
        self.status = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            (self.callback)(&envelope)
        })) {
            Ok(Ok(CandidateRoutedObserverControl::Continue)) => {
                CandidateRoutedObserverStatus::Active
            }
            Ok(Ok(CandidateRoutedObserverControl::Detach)) => {
                CandidateRoutedObserverStatus::Detached
            }
            Ok(Err(())) => {
                CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::CallbackError)
            }
            Err(payload) => {
                // A user panic payload can panic during Drop, too. Isolate
                // that destruction from reduction unwinding; a second payload
                // cannot safely be inspected or dropped here.
                if let Err(drop_panic) =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        drop(payload);
                    }))
                {
                    std::mem::forget(drop_panic);
                }
                CandidateRoutedObserverStatus::Faulted(CandidateRoutedObserverFault::Panicked)
            }
        };
    }
}

trait AdvancedObserver {
    fn options(&self) -> CandidateRoutedObservationOptions;
    fn enabled(&self) -> bool;
    fn emit(&mut self, envelope: CandidateRoutedApplicationEnvelope<'_>);
}
impl AdvancedObserver for CandidateRoutedApplicationObserver<'_> {
    fn options(&self) -> CandidateRoutedObservationOptions {
        self.options()
    }
    fn enabled(&self) -> bool {
        self.status() == CandidateRoutedObserverStatus::Active
    }
    fn emit(&mut self, envelope: CandidateRoutedApplicationEnvelope<'_>) {
        self.emit(envelope);
    }
}
enum Observer<'a> {
    Legacy(&'a mut dyn FnMut(&CandidateRoutedApplicationSnapshot)),
    Advanced(&'a mut dyn AdvancedObserver),
}
pub(super) struct Progress<'a> {
    observer: Observer<'a>,
    pub phase: CandidateRoutedApplicationPhase,
    pub started: Instant,
    pub sequence: u64,
    interval: Duration,
    last: Instant,
    remaining: u16,
}

impl<'a> Progress<'a> {
    pub fn new(
        interval: Duration,
        observer: &'a mut dyn FnMut(&CandidateRoutedApplicationSnapshot),
    ) -> Self {
        let started = Instant::now();
        Self {
            observer: Observer::Legacy(observer),
            phase: CandidateRoutedApplicationPhase::Admission,
            started,
            sequence: 0,
            interval,
            last: started,
            remaining: 256,
        }
    }

    pub fn from_observer(observer: &'a mut CandidateRoutedApplicationObserver<'_>) -> Option<Self> {
        if observer.status() != CandidateRoutedObserverStatus::Active {
            return None;
        }
        let options = observer.options();
        let started = Instant::now();
        Some(Self {
            observer: Observer::Advanced(observer),
            phase: CandidateRoutedApplicationPhase::Admission,
            started,
            sequence: 0,
            interval: options.interval,
            last: started,
            remaining: 256,
        })
    }
    pub fn is_advanced(&self) -> bool {
        matches!(&self.observer, Observer::Advanced(observer) if observer.enabled())
    }
    pub fn options(&self) -> CandidateRoutedObservationOptions {
        match &self.observer {
            Observer::Advanced(observer) => observer.options(),
            Observer::Legacy(_) => CandidateRoutedObservationOptions {
                interval: self.interval,
                max_frontier_frames: 0,
                max_path_nodes: 0,
            },
        }
    }
    pub fn deliver(
        &mut self,
        snapshot: &CandidateRoutedApplicationSnapshot,
        mut envelope: CandidateRoutedApplicationEnvelope<'_>,
    ) {
        match &mut self.observer {
            Observer::Legacy(observer) => observer(snapshot),
            Observer::Advanced(observer) => {
                envelope.snapshot = Some(*snapshot);
                envelope.status = snapshot.status;
                observer.emit(envelope);
            }
        }
    }

    /// Lightweight activity does not reset the quantitative-snapshot clock or
    /// work gate: frequent activity must not starve frontier/complexity samples.
    pub fn notice(&mut self) -> Option<Instant> {
        if !self.is_advanced() {
            return None;
        }
        self.sequence = self.sequence.saturating_add(1);
        Some(Instant::now())
    }

    pub fn due(&mut self, force: bool) -> Option<Instant> {
        if matches!(&self.observer, Observer::Advanced(observer) if !observer.enabled()) {
            return None;
        }
        if !force && !self.interval.is_zero() {
            self.remaining -= 1;
            if self.remaining != 0 {
                return None;
            }
            self.remaining = 256;
        }
        let now = Instant::now();
        if !force && now.duration_since(self.last) < self.interval {
            return None;
        }
        self.last = now;
        self.sequence = self.sequence.saturating_add(1);
        Some(now)
    }
}

#[cfg(test)]
mod advanced_tests;
#[cfg(test)]
mod tests;
