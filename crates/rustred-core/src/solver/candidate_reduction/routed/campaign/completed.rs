//! Complete-only, process-local dependency reuse. No serialization authority,
//! coefficient cache, terminal refinement, or replacement algebra kernel.
use super::scheduler::membership::Membership;
use super::*;
use std::collections::BTreeSet;
use std::sync::atomic::Ordering;

/// Explicit bounds on retained closure and the atomic merge's live payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedCompletedTraceLimits {
    pub max_completed_work: usize,
    /// Old + delta membership key payload plus old + delta + staged raw
    /// terminal/zero/census payload. Hash slack, tree nodes, allocator overhead,
    /// prepared programs and caller-retained reports are NOT process RSS. The
    /// Work and census-count grants bound container growth independently.
    pub max_merge_payload_bytes: usize,
}

/// The successful raw closure of every completed batch in one exact immutable
/// reducer. Its private membership is never minted from counters, local `done`,
/// public snapshots, arbitrary keys, or a deserialized flag. It has no Clone.
///
/// A shared borrow binds owners, routes, order, guards and raw terminal authority
/// to this precise reducer instance. Downstream master substitutions are not
/// applied here. New batches still validate all starts before dependency hits.
pub struct CandidateRoutedCompletedTrace<'r, const N: usize> {
    reducer: &'r RoutedCandidateReducer<N>,
    membership: Membership<N>,
    terminals: BTreeSet<IntegralKey>,
    zeros: BTreeSet<IntegralKey>,
    census: CandidateRoutedTraceCensus,
    completed_work: usize,
    completed_batches: usize,
    input_occurrences: usize,
    membership_payload_bound: usize,
    budget: CandidateRoutedWorkBudget,
    census_limits: CandidateRoutedTraceCensusLimits,
    limits: CandidateRoutedCompletedTraceLimits,
}

impl<const N: usize> std::fmt::Debug for CandidateRoutedCompletedTrace<'_, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidateRoutedCompletedTrace")
            .field("completed_work", &self.completed_work)
            .field("completed_batches", &self.completed_batches)
            .field("input_occurrences", &self.input_occurrences)
            .field("terminals", &self.terminals.len())
            .field("zeros", &self.zeros.len())
            .finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub enum CandidateRoutedCompletedTraceError<const N: usize> {
    Trace(CandidateRoutedCampaignError<N>),
    /// The native scheduler drained, but an unresolved leaf is not a master.
    Frontier(CandidateRoutedCampaignReport<N>),
    Retention {
        reason: super::super::CandidateRoutedError,
        report: CandidateRoutedCampaignReport<N>,
    },
}
impl<const N: usize> CandidateRoutedCompletedTraceError<N> {
    pub fn report(&self) -> &CandidateRoutedCampaignReport<N> {
        match self {
            Self::Trace(error) => error.partial_report(),
            Self::Frontier(report) | Self::Retention { report, .. } => report,
        }
    }
}
impl<const N: usize> std::fmt::Display for CandidateRoutedCompletedTraceError<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Trace(error) => error.fmt(f),
            Self::Frontier(_) => f.write_str("completed trace has unresolved frontier"),
            Self::Retention { reason, .. } => write!(f, "completed trace not committed: {reason}"),
        }
    }
}
impl<const N: usize> std::error::Error for CandidateRoutedCompletedTraceError<N> {}

impl<const N: usize> RoutedCandidateReducer<N> {
    /// Begin complete-only sharing with one fully validated bounded batch.
    /// The returned report describes this invocation; the opaque capability
    /// holds cumulative raw closure. This does not certify source derivations
    /// or linear independence of the raw terminal convention.
    pub fn trace_targets_completed_with_entry_admission_and_observer(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        budget: CandidateRoutedWorkBudget,
        census_limits: CandidateRoutedTraceCensusLimits,
        limits: CandidateRoutedCompletedTraceLimits,
        cancellation: &AtomicBool,
        observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<
        (
            CandidateRoutedCompletedTrace<'_, N>,
            CandidateRoutedCampaignReport<N>,
        ),
        CandidateRoutedCompletedTraceError<N>,
    > {
        // This unsealed value cannot escape: only trace_batch success returns it.
        let mut completed = CandidateRoutedCompletedTrace {
            reducer: self,
            membership: Membership::new(),
            terminals: BTreeSet::new(),
            zeros: BTreeSet::new(),
            census: CandidateRoutedTraceCensus::default(),
            completed_work: 0,
            completed_batches: 0,
            input_occurrences: 0,
            membership_payload_bound: 0,
            budget,
            census_limits,
            limits,
        };
        let report = completed.trace_batch(targets, admission, workers, cancellation, observer)?;
        Ok((completed, report))
    }
}

impl<const N: usize> CandidateRoutedCompletedTrace<'_, N> {
    pub fn completed_work(&self) -> usize {
        self.completed_work
    }
    pub fn completed_batches(&self) -> usize {
        self.completed_batches
    }
    /// Sum of admitted input occurrences, NOT distinct starts across batches.
    pub fn input_occurrences(&self) -> usize {
        self.input_occurrences
    }
    pub fn declared_terminals(&self) -> &BTreeSet<IntegralKey> {
        &self.terminals
    }
    pub fn visited_zeros(&self) -> &BTreeSet<IntegralKey> {
        &self.zeros
    }
    pub fn census(&self) -> &CandidateRoutedTraceCensus {
        &self.census
    }
    pub fn contains_work(&self, work: &CandidateRoutedWork<N>) -> bool {
        self.membership.probe(work).1
    }

    /// Execute a fresh delta against the earlier complete dependency graph.
    /// On any failure, cancellation or frontier, semantic baseline membership,
    /// census and completion counters are unchanged. A failed reservation may
    /// increase HashMap capacity, never insert keys or grant closure.
    ///
    /// Aggregate native budgets and cumulative grants remain fixed from the
    /// first batch. Workers may vary without changing per-key dispatch. The
    /// final observer notification precedes the atomic semantic commit; a
    /// cancellation there discards the delta. No callback/fallible operation
    /// occurs after the final cancellation check at the commit linearization.
    /// Metadata staging is O(cumulative terminal/census size) per batch, while
    /// closed Work is moved without a full copy. This first RAM implementation
    /// is not a bounded-storage solution for arbitrarily many windows. Report
    /// elapsed time covers native tracing; caller wall time also includes merge.
    pub fn trace_batch(
        &mut self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        cancellation: &AtomicBool,
        mut observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCampaignReport<N>, CandidateRoutedCompletedTraceError<N>> {
        let (report, delta) = self
            .reducer
            .trace_parallel(
                targets,
                admission,
                workers,
                Some(self.budget),
                Some(self.census_limits),
                (self.completed_work != 0).then_some(&self.membership),
                cancellation,
                &mut observer,
            )
            .map_err(CandidateRoutedCompletedTraceError::Trace)?;
        if !report.trace.frontier.is_empty() {
            return Err(CandidateRoutedCompletedTraceError::Frontier(report));
        }
        let Some(census) = report.complete_census() else {
            return Err(retention_error(
                report,
                super::super::CandidateRoutedError::InvalidInput(
                    "complete-only trace lacks finished native census".into(),
                ),
            ));
        };
        let mut check = || {
            if cancellation.load(Ordering::Acquire) {
                Err(super::super::CandidateRoutedError::InvalidInput(
                    "completed trace staging cancelled".into(),
                ))
            } else {
                Ok(())
            }
        };
        let staged = (|| {
            check()?;
            let work = checked(
                self.completed_work,
                report.snapshot.scheduled_nodes,
                self.limits.max_completed_work,
                "retained completed Work",
            )?;
            let batches = checked(
                self.completed_batches,
                1,
                usize::MAX,
                "completed batch counter",
            )?;
            let occurrences = checked(
                self.input_occurrences,
                report.snapshot.input_targets,
                usize::MAX,
                "completed input occurrence counter",
            )?;
            let delta_bytes = delta.payload_bytes(&mut check)?;
            let membership_bytes = checked(
                self.membership_payload_bound,
                delta_bytes,
                self.limits.max_merge_payload_bytes,
                "completed membership payload",
            )?;
            // Conservative upper bound before any metadata clone: the staged
            // union cannot contain more payload than old plus delta metadata.
            let old = metadata_payload(&self.terminals, &self.zeros, &self.census, &mut check)?;
            let new = metadata_payload(
                &report.trace.declared_terminals,
                &report.trace.visited_zeros,
                census,
                &mut check,
            )?;
            let metadata = checked(old, new, usize::MAX, "completed merge metadata")?;
            checked(
                membership_bytes,
                checked(metadata, metadata, usize::MAX, "completed staged metadata")?,
                self.limits.max_merge_payload_bytes,
                "completed merge live payload",
            )?;
            let census = self.census.merged(census, self.census_limits, &mut check)?;
            let mut terminals = BTreeSet::new();
            for key in self
                .terminals
                .iter()
                .chain(&report.trace.declared_terminals)
            {
                check()?;
                terminals.insert(key.clone());
            }
            let mut zeros = BTreeSet::new();
            for key in self.zeros.iter().chain(&report.trace.visited_zeros) {
                check()?;
                zeros.insert(key.clone());
            }
            self.membership.reserve_completed(&delta, &mut check)?;
            check()?;
            Ok((
                work,
                batches,
                occurrences,
                membership_bytes,
                terminals,
                zeros,
                census,
            ))
        })();
        let (work, batches, occurrences, membership_bytes, terminals, zeros, census) = match staged
        {
            Ok(staged) => staged,
            Err(error) => {
                return Err(if cancellation.load(Ordering::Acquire) {
                    cancelled(report)
                } else {
                    retention_error(report, error)
                });
            }
        };
        observer(report.snapshot());
        if cancellation.load(Ordering::Acquire) {
            return Err(cancelled(report));
        }
        self.membership.append_completed(delta);
        self.terminals = terminals;
        self.zeros = zeros;
        self.census = census;
        self.completed_work = work;
        self.completed_batches = batches;
        self.input_occurrences = occurrences;
        self.membership_payload_bound = membership_bytes;
        Ok(report)
    }
}

fn cancelled<const N: usize>(
    mut report: CandidateRoutedCampaignReport<N>,
) -> CandidateRoutedCompletedTraceError<N> {
    report.snapshot.finished = false;
    report.snapshot.first_failure = Some(CandidateRoutedCampaignFailure::Cancelled);
    CandidateRoutedCompletedTraceError::Trace(CandidateRoutedCampaignError {
        reason: CandidateRoutedCampaignFailure::Cancelled,
        partial: report,
    })
}

fn retention_error<const N: usize>(
    mut report: CandidateRoutedCampaignReport<N>,
    reason: super::super::CandidateRoutedError,
) -> CandidateRoutedCompletedTraceError<N> {
    report.snapshot.finished = false;
    CandidateRoutedCompletedTraceError::Retention { reason, report }
}
fn quota(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> super::super::CandidateRoutedError {
    super::super::CandidateRoutedError::ResourceLimit {
        resource,
        requested,
        limit,
    }
}
fn checked(
    value: usize,
    addition: usize,
    maximum: usize,
    resource: &'static str,
) -> Result<usize, super::super::CandidateRoutedError> {
    let requested = value
        .checked_add(addition)
        .ok_or_else(|| quota(resource, usize::MAX, maximum))?;
    if requested > maximum {
        Err(quota(resource, requested, maximum))
    } else {
        Ok(requested)
    }
}
fn metadata_payload(
    terminals: &BTreeSet<IntegralKey>,
    zeros: &BTreeSet<IntegralKey>,
    census: &CandidateRoutedTraceCensus,
    check: &mut dyn FnMut() -> Result<(), super::super::CandidateRoutedError>,
) -> Result<usize, super::super::CandidateRoutedError> {
    let mut total = census.identity_payload_bytes();
    for key in terminals.iter().chain(zeros) {
        check()?;
        let coordinates = key
            .powers()
            .len()
            .checked_mul(std::mem::size_of::<i64>())
            .ok_or_else(|| quota("completed key payload", usize::MAX, usize::MAX))?;
        total = checked(
            total,
            std::mem::size_of::<IntegralKey>(),
            usize::MAX,
            "completed key payload",
        )?;
        total = checked(total, coordinates, usize::MAX, "completed key payload")?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests;
