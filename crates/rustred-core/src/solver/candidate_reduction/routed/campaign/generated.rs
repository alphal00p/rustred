//! Explicit trusted-generated dependency reuse. Native-only CompletedTrace
//! remains separate: hashes and structural validation do not prove closure.
use super::*;
use crate::persistence::finite_work::{
    GeneratedExecutionBinding, GeneratedPageProvider, GeneratedWorkDigest, GeneratedWorkError,
    GeneratedWorkErrorKind, GeneratedWorkKey, TrustedGeneratedWorkDirectory,
};
use std::sync::atomic::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CandidateRoutedDeltaExportLimits {
    pub max_records: usize,
    /// Delta membership plus exported full-phase-key payload. Includes the
    /// extra powers when both Route/Apply occur, but not allocator/hash slack,
    /// report metadata, provider residency or process RSS. Sort is in-place.
    pub max_live_payload_bytes: usize,
}

/// An explicitly TRUSTED-GENERATED predecessor joined to this exact immutable
/// reducer. Its construction is not an independent native closure proof.
pub struct CandidateRoutedGeneratedBaseline<'a, const N: usize> {
    reducer: &'a RoutedCandidateReducer<N>,
    directory: &'a TrustedGeneratedWorkDirectory,
    provider: &'a dyn GeneratedPageProvider,
    budget: CandidateRoutedWorkBudget,
    census_limits: CandidateRoutedTraceCensusLimits,
    export_limits: CandidateRoutedDeltaExportLimits,
}

/// New native-completed Work, RELATIVE to the captured trusted predecessor.
/// No Clone/Decode/public constructor: failed, frontier, cancelled or partially
/// drained invocations cannot mint it. It is not yet a committed generation.
pub struct CandidateRoutedCompletedDelta<'a, const N: usize> {
    reducer: &'a RoutedCandidateReducer<N>,
    execution: GeneratedExecutionBinding,
    predecessor: GeneratedWorkDigest,
    work: Vec<GeneratedWorkKey>,
    report: CandidateRoutedCampaignReport<N>,
}
impl<const N: usize> std::fmt::Debug for CandidateRoutedCompletedDelta<'_, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CandidateRoutedCompletedDelta")
            .field("predecessor", &self.predecessor)
            .field("work", &self.work.len())
            .finish_non_exhaustive()
    }
}
impl<const N: usize> CandidateRoutedCompletedDelta<'_, N> {
    pub fn predecessor(&self) -> GeneratedWorkDigest {
        self.predecessor
    }
    pub fn execution(&self) -> &GeneratedExecutionBinding {
        &self.execution
    }
    pub fn work(&self) -> &[GeneratedWorkKey] {
        &self.work
    }
    pub fn report(&self) -> &CandidateRoutedCampaignReport<N> {
        &self.report
    }
    pub fn is_for_reducer(&self, reducer: &RoutedCandidateReducer<N>) -> bool {
        std::ptr::eq(self.reducer, reducer)
    }
    /// Consume the seal after the persistence adapter has checked predecessor
    /// and exact reducer identity. These plain data parts alone carry no seal.
    pub fn into_parts(self) -> (Vec<GeneratedWorkKey>, CandidateRoutedCampaignReport<N>) {
        (self.work, self.report)
    }
}
impl<const N: usize> RoutedCandidateReducer<N> {
    /// Structural validation of a trusted generated census identity against
    /// the actual immutable owner batches. This does not replay applicability.
    pub fn validate_generated_selection(
        &self,
        owner: &[bool],
        batch: usize,
        baseline: usize,
        selected: usize,
    ) -> Result<(), GeneratedWorkError> {
        let arity = self.programs.context.family.denominator_count();
        if owner.len() != arity {
            return Err(invalid("generated census owner arity"));
        }
        let mask = std::array::from_fn(|i| owner.get(i).copied().unwrap_or(false));
        let Some(batch) = self
            .programs
            .owners
            .get(&mask)
            .and_then(|owner| owner.batches.get(batch))
        else {
            return Err(invalid("generated census owner/batch absent"));
        };
        if !batch.rules.iter().any(|rule| rule.ordinal == baseline)
            || !batch.rules.iter().any(|rule| rule.ordinal == selected)
        {
            return Err(invalid("generated census saved ordinal absent"));
        }
        if baseline != selected
            && !batch
                .whole_piece_alternatives
                .iter()
                .any(|&index| batch.rules[index].ordinal == selected)
        {
            return Err(invalid(
                "generated census alternative is not an installed whole-piece choice",
            ));
        }
        Ok(())
    }
    pub fn validate_generated_terminal(&self, key: &IntegralKey) -> Result<(), GeneratedWorkError> {
        if key.powers().len() != self.programs.context.family.denominator_count() {
            return Err(invalid("generated terminal arity"));
        }
        let mask = std::array::from_fn(|i| key.powers().get(i).is_some_and(|&n| n > 0));
        if !self.programs.owners.get(&mask).is_some_and(|owner| {
            owner
                .batches
                .iter()
                .any(|batch| batch.terminals.contains(key))
        }) {
            return Err(invalid(
                "generated terminal is not declared by the raw owner programs",
            ));
        }
        Ok(())
    }
    pub fn validate_generated_zero(&self, key: &IntegralKey) -> Result<(), GeneratedWorkError> {
        if key.powers().len() != self.programs.context.family.denominator_count()
            || !worker::base(self).is_zero(key)
        {
            return Err(invalid("generated zero is not in the source zero sectors"));
        }
        Ok(())
    }
    /// Explicit trusted-checkpoint boundary. `authenticated_execution` must
    /// come from authenticating the SAME source bytes/policies that prepared
    /// this reducer, not an arbitrary digest supplied after preparation. The
    /// app's private prepared-source adapter enforces that association. Core
    /// validates the captured directory identity/arity and performs all exact
    /// membership decisions; it does not infer historical closure from hashes.
    pub fn bind_trusted_generated_baseline<'a>(
        &'a self,
        authenticated_execution: &GeneratedExecutionBinding,
        directory: &'a TrustedGeneratedWorkDirectory,
        provider: &'a dyn GeneratedPageProvider,
        budget: CandidateRoutedWorkBudget,
        census_limits: CandidateRoutedTraceCensusLimits,
        export_limits: CandidateRoutedDeltaExportLimits,
    ) -> Result<CandidateRoutedGeneratedBaseline<'a, N>, GeneratedWorkError> {
        if directory.execution() != authenticated_execution
            || directory.execution().physical_arity as usize
                != self.programs.context.family.denominator_count()
        {
            return Err(GeneratedWorkError::new(
                GeneratedWorkErrorKind::Corrupt,
                "trusted generated baseline differs from prepared execution/arity",
            ));
        }
        Ok(CandidateRoutedGeneratedBaseline {
            reducer: self,
            directory,
            provider,
            budget,
            census_limits,
            export_limits,
        })
    }
}
fn invalid(message: &str) -> GeneratedWorkError {
    GeneratedWorkError::new(GeneratedWorkErrorKind::Corrupt, message)
}
impl<'a, const N: usize> CandidateRoutedGeneratedBaseline<'a, N> {
    /// A successful result contains ONLY new Work/census/terminal/zero deltas.
    /// Prior raw closure stays in the predecessor. Hits are dependencies, never
    /// terminals. All starts are admitted before lookup, including an all-hit
    /// batch. Persistence failure must prevent advancing to another window.
    pub fn trace_batch(
        &self,
        targets: impl IntoIterator<Item = IntegralKey>,
        admission: CandidateEntryAdmission<'_, N>,
        workers: usize,
        cancellation: &AtomicBool,
        mut observer: impl FnMut(&CandidateRoutedCampaignSnapshot<N>),
    ) -> Result<CandidateRoutedCompletedDelta<'a, N>, CandidateRoutedCompletedTraceError<N>> {
        let baseline = (self.directory.statistics().records != 0).then_some(
            scheduler::CompletedBaseline::Generated {
                directory: self.directory,
                provider: self.provider,
            },
        );
        let (report, delta) = self
            .reducer
            .trace_parallel(
                targets,
                admission,
                workers,
                Some(self.budget),
                Some(self.census_limits),
                baseline,
                cancellation,
                &mut observer,
            )
            .map_err(CandidateRoutedCompletedTraceError::Trace)?;
        if !report.trace.frontier.is_empty() {
            return Err(CandidateRoutedCompletedTraceError::Frontier(report));
        }
        if report.complete_census().is_none() {
            return Err(completed::retention_error(
                report,
                super::super::CandidateRoutedError::InvalidInput(
                    "generated delta lacks finished native census".into(),
                ),
            ));
        }
        let mut check = || {
            if cancellation.load(Ordering::Acquire) {
                Err(super::super::CandidateRoutedError::InvalidInput(
                    "generated delta export cancelled".into(),
                ))
            } else {
                Ok(())
            }
        };
        let work = match delta.into_generated_work(
            report.snapshot.scheduled_nodes,
            self.export_limits,
            &mut check,
        ) {
            Ok(work) => work,
            Err(error) => {
                return Err(if cancellation.load(Ordering::Acquire) {
                    completed::cancelled(report)
                } else {
                    completed::retention_error(report, error)
                });
            }
        };
        observer(report.snapshot());
        if cancellation.load(Ordering::Acquire) {
            return Err(completed::cancelled(report));
        }
        Ok(CandidateRoutedCompletedDelta {
            reducer: self.reducer,
            execution: self.directory.execution().clone(),
            predecessor: self.directory.generation(),
            work,
            report,
        })
    }
}

#[cfg(test)]
mod tests;
