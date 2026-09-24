//! Optional exact reuse of a high-D slice of an immutable initial Apply domain.
//! Inclusion is native geometry; coverage remains a pinned ledger obligation.
use std::collections::{HashMap, HashSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use rustred::solver::{DomainPowerBounds, DomainPowerSummary};

use super::queue::{Domain, Phase};

pub(super) const MAX_INITIAL_DOMAINS: usize = 4096;
pub(super) const MAX_ENTRY_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InitialOverlapBuildStatus {
    NotRequested,
    Active,
    EmptyInitial,
    NoEligibleApply,
    NoUsableAnchors,
    CountLimit,
    ByteLimit,
    PayloadOverflow,
    AllocationFailure,
    Cancelled,
}

impl InitialOverlapBuildStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotRequested => "not_requested",
            Self::Active => "active",
            Self::EmptyInitial => "empty_initial",
            Self::NoEligibleApply => "no_eligible_apply",
            Self::NoUsableAnchors => "no_usable_anchors",
            Self::CountLimit => "count_limit",
            Self::ByteLimit => "byte_limit",
            Self::PayloadOverflow => "payload_overflow",
            Self::AllocationFailure => "allocation_failure",
            Self::Cancelled => "cancelled",
        }
    }
}

/// Optional-index build diagnostics, never coverage or completion authority.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InitialOverlapBuildReport {
    pub total_initial: usize,
    pub examined_initial: usize,
    /// A prefix count unless eligibility_complete is true.
    pub eligible_apply: usize,
    pub eligibility_complete: bool,
    pub retained_membership: usize,
    pub usable_anchors: usize,
    pub logical_bytes_per_eligible_entry: usize,
    /// Requested conservative logical entry charge, not retained allocation or
    /// RSS. None for incomplete eligibility counting or multiplication overflow.
    /// A refused/failed build can have a requested charge but retains no index.
    pub requested_logical_entry_bytes: Option<usize>,
    pub status: InitialOverlapBuildStatus,
}

/// Original coordinates/rank are unchanged. Only residual D bounds differ.
/// This is partial native work, never a whole-domain alias or a solved anchor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct InitialOverlapScope {
    pub anchor_id: usize,
    pub cut: i64,
    pub residual_powers: DomainPowerBounds,
}

pub(super) struct InitialOverlapPlan<const N: usize> {
    pub scope: InitialOverlapScope,
    pub residual: Domain<N>,
}

struct Anchor<const N: usize> {
    id: usize,
    cut: i64,
    summary: DomainPowerSummary<N>,
}

pub(super) struct InitialOverlapIndex<const N: usize> {
    // Complete Apply membership or empty: even initial Apply entries omitted
    // from the anchor lists must bypass pruning. Route cannot use plan(). The
    // persistent queue exact map forbids a later identical raw domain from
    // receiving a second ID. The queue/ledger still retain every phase.
    initial: HashSet<Arc<Domain<N>>>,
    anchors: HashMap<[bool; N], Vec<Anchor<N>>>,
    report: InitialOverlapBuildReport,
}

impl<const N: usize> InitialOverlapIndex<N> {
    pub fn empty() -> Self {
        Self {
            initial: HashSet::new(),
            anchors: HashMap::new(),
            report: InitialOverlapBuildReport {
                total_initial: 0,
                examined_initial: 0,
                eligible_apply: 0,
                eligibility_complete: false,
                retained_membership: 0,
                usable_anchors: 0,
                logical_bytes_per_eligible_entry: Self::entry_bytes(),
                requested_logical_entry_bytes: None,
                status: InitialOverlapBuildStatus::NotRequested,
            },
        }
    }

    pub fn build_report(&self) -> InitialOverlapBuildReport {
        self.report
    }

    fn entry_bytes() -> usize {
        // Logical entry payload only, not allocator overhead or RSS. Domain
        // coordinate allocations are shared by Arc, never cloned into the index.
        size_of::<Arc<Domain<N>>>()
            + size_of::<Anchor<N>>()
            + size_of::<([bool; N], Vec<Anchor<N>>)>()
    }

    fn disabled(mut report: InitialOverlapBuildReport, status: InitialOverlapBuildStatus) -> Self {
        report.status = status;
        report.retained_membership = 0;
        report.usable_anchors = 0;
        Self {
            initial: HashSet::new(),
            anchors: HashMap::new(),
            report,
        }
    }

    /// Caller has already pinned this actual initial prefix before its first
    /// admission and frozen it in the queue-local responsibility ledger.
    pub fn from_initial(domains: &[Arc<Domain<N>>], cancellation: &AtomicBool) -> Self {
        Self::with_limits(domains, cancellation, MAX_INITIAL_DOMAINS, MAX_ENTRY_BYTES)
    }

    fn with_limits(
        domains: &[Arc<Domain<N>>],
        cancellation: &AtomicBool,
        max_domains: usize,
        max_bytes: usize,
    ) -> Self {
        Self::build(
            domains,
            max_domains,
            max_bytes,
            || cancellation.load(Ordering::Acquire),
            || true,
        )
    }

    /// Small private control seams make cancellation in either pass and reserve
    /// fallback deterministic in tests. Production reads the caller's atomic and
    /// admits each ordinary try_reserve; no global allocator hook is installed.
    fn build(
        domains: &[Arc<Domain<N>>],
        max_domains: usize,
        max_bytes: usize,
        mut cancelled: impl FnMut() -> bool,
        mut allow_reserve: impl FnMut() -> bool,
    ) -> Self {
        let mut out = Self::empty();
        out.report.total_initial = domains.len();
        for domain in domains {
            if cancelled() {
                return Self::disabled(out.report, InitialOverlapBuildStatus::Cancelled);
            }
            out.report.examined_initial += 1;
            if domain.phase == Phase::Apply {
                out.report.eligible_apply += 1;
            }
        }
        out.report.eligibility_complete = true;
        let eligible = out.report.eligible_apply;
        out.report.requested_logical_entry_bytes =
            eligible.checked_mul(out.report.logical_bytes_per_eligible_entry);
        if cancelled() {
            return Self::disabled(out.report, InitialOverlapBuildStatus::Cancelled);
        }
        if domains.is_empty() {
            return Self::disabled(out.report, InitialOverlapBuildStatus::EmptyInitial);
        }
        if eligible == 0 {
            return Self::disabled(out.report, InitialOverlapBuildStatus::NoEligibleApply);
        }
        if eligible > max_domains {
            return Self::disabled(out.report, InitialOverlapBuildStatus::CountLimit);
        }
        let Some(requested_bytes) = out.report.requested_logical_entry_bytes else {
            return Self::disabled(out.report, InitialOverlapBuildStatus::PayloadOverflow);
        };
        if requested_bytes > max_bytes {
            return Self::disabled(out.report, InitialOverlapBuildStatus::ByteLimit);
        }
        if !allow_reserve()
            || out.initial.try_reserve(eligible).is_err()
            || !allow_reserve()
            || out.anchors.try_reserve(eligible).is_err()
        {
            return Self::disabled(out.report, InitialOverlapBuildStatus::AllocationFailure);
        }
        for (id, domain) in domains.iter().enumerate() {
            if cancelled() {
                return Self::disabled(out.report, InitialOverlapBuildStatus::Cancelled);
            }
            if domain.phase != Phase::Apply {
                continue;
            }
            // Membership includes every initial Apply descriptor even when its
            // summary/cut cannot be used, and IDs refer to the original prefix.
            out.initial.insert(domain.clone());
            let Ok(summary) = summary(domain, domain.powers) else {
                continue;
            };
            let Some(cut) = summary
                .extrema()
                .and_then(|x| x.power_difference().0)
                .and_then(|x| i64::try_from(x).ok())
            else {
                continue;
            };
            if cut.checked_sub(1).is_none() {
                continue;
            }
            let anchors = out.anchors.entry(domain.owner).or_default();
            if !allow_reserve() || anchors.try_reserve(1).is_err() {
                return Self::disabled(out.report, InitialOverlapBuildStatus::AllocationFailure);
            }
            anchors.push(Anchor { id, cut, summary });
            out.report.usable_anchors += 1;
        }
        if cancelled() {
            return Self::disabled(out.report, InitialOverlapBuildStatus::Cancelled);
        }
        out.report.retained_membership = out.initial.len();
        out.report.status = if out.report.usable_anchors == 0 {
            InitialOverlapBuildStatus::NoUsableAnchors
        } else {
            InitialOverlapBuildStatus::Active
        };
        out
    }

    /// Deterministic first native-valid cut in initial admission order.
    /// Optional failures never skip work: the caller inspects the original Q.
    pub fn plan(
        &self,
        domain: &Domain<N>,
        cancellation: &AtomicBool,
    ) -> Option<InitialOverlapPlan<N>> {
        if domain.phase != Phase::Apply || self.initial.contains(domain) {
            return None;
        }
        let anchors = self.anchors.get(&domain.owner)?;
        // Validate the unmodified descriptor first. An inverted generated
        // intersection is an empty split, not permission to mask bad input.
        if summary(domain, domain.powers).ok()?.is_empty() {
            return None;
        }
        for anchor in anchors {
            if cancellation.load(Ordering::Acquire) {
                return None;
            }
            let below = anchor.cut.checked_sub(1)?;
            let mut high = domain.powers;
            high.min_power_difference = Some(
                high.min_power_difference
                    .map_or(anchor.cut, |v| v.max(anchor.cut)),
            );
            let mut low = domain.powers;
            low.max_power_difference =
                Some(low.max_power_difference.map_or(below, |v| v.min(below)));
            if inverted(high) || inverted(low) {
                continue;
            }
            let (Ok(high_summary), Ok(low_summary)) = (summary(domain, high), summary(domain, low))
            else {
                continue;
            };
            if high_summary.is_empty()
                || low_summary.is_empty()
                || !anchor.summary.contains(&high_summary)
            {
                continue;
            }
            let mut lower = Vec::new();
            let mut upper = Vec::new();
            if lower.try_reserve_exact(domain.lower.len()).is_err()
                || upper.try_reserve_exact(domain.upper.len()).is_err()
            {
                return None;
            }
            lower.extend_from_slice(&domain.lower);
            upper.extend_from_slice(&domain.upper);
            return Some(InitialOverlapPlan {
                scope: InitialOverlapScope {
                    anchor_id: anchor.id,
                    cut: anchor.cut,
                    residual_powers: low,
                },
                residual: Domain {
                    phase: domain.phase,
                    owner: domain.owner,
                    lower,
                    upper,
                    rank: domain.rank,
                    powers: low,
                },
            });
        }
        None
    }
}

fn inverted(bounds: DomainPowerBounds) -> bool {
    matches!((bounds.min_power_difference, bounds.max_power_difference), (Some(a), Some(b)) if a > b)
}

fn summary<const N: usize>(
    domain: &Domain<N>,
    powers: DomainPowerBounds,
) -> Result<DomainPowerSummary<N>, rustred::solver::DomainPowerError> {
    DomainPowerSummary::try_new(
        domain.owner,
        &domain.lower,
        &domain.upper,
        domain.rank,
        powers,
    )
}

#[cfg(test)]
mod tests;
