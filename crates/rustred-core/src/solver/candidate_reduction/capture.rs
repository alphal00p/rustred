//! Native candidate capture events. Algebra and normalization stay in the
//! existing candidate evaluator; only completed exact results are exported.
use super::{
    CandidateGuardedCacheError, CandidateReducer, CandidateReductionError, CandidateUnitMassProgram,
};
use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::persistence::application_cache::{ApplicationCacheKey, ApplicationCacheRecord};
use crate::reduction::guarded_capture::GuardProvenance;
use crate::reduction::{
    GuardedCacheError, GuardedCaptureStatistics, GuardedIntermediateCapture, ReductionLimits,
    SharedCacheBudget,
};
use std::sync::Arc;

pub(super) trait CaptureObserver<const N: usize, E> {
    fn local(&mut self, key: &IntegralKey, conditions: &[Coefficient]) -> Result<(), E>;
    fn child(&mut self, parent: &IntegralKey, child: &IntegralKey) -> Result<(), E>;
    fn completed(
        &mut self,
        key: &IntegralKey,
        source: &CandidateReducer<N>,
        imported: bool,
    ) -> Result<(), E>;
    fn root_conditions(&self) -> Vec<Coefficient>;
}

pub(super) struct CandidateCapture<'a, const N: usize> {
    graph: GuardProvenance,
    program: &'a CandidateUnitMassProgram<N>,
    root: &'a IntegralKey,
    sink: &'a mut dyn GuardedIntermediateCapture,
    limits: ReductionLimits,
    captured: usize,
}
impl<'a, const N: usize> CandidateCapture<'a, N> {
    pub(super) fn new(
        program: &'a CandidateUnitMassProgram<N>,
        root: &'a IntegralKey,
        sink: &'a mut dyn GuardedIntermediateCapture,
        shared: Arc<SharedCacheBudget>,
        limits: ReductionLimits,
    ) -> Self {
        Self {
            graph: GuardProvenance::new(shared, limits, sink.metadata_limit_bytes()),
            program,
            root,
            sink,
            limits,
            captured: 0,
        }
    }
    pub(super) fn statistics(&self) -> GuardedCaptureStatistics {
        let mut statistics = self.graph.statistics();
        statistics.captured_records = self.captured;
        statistics
    }
    pub(super) fn report(&mut self) {
        self.sink.report_statistics(self.statistics());
    }
}
impl<const N: usize> CaptureObserver<N, CandidateGuardedCacheError> for CandidateCapture<'_, N> {
    fn local(
        &mut self,
        key: &IntegralKey,
        conditions: &[Coefficient],
    ) -> Result<(), CandidateGuardedCacheError> {
        let node = self.graph.node_for(0, key)?;
        self.graph.add_local(node, conditions)?;
        Ok(())
    }
    fn child(
        &mut self,
        parent: &IntegralKey,
        child: &IntegralKey,
    ) -> Result<(), CandidateGuardedCacheError> {
        let parent = self.graph.node_for(0, parent)?;
        let child = self.graph.node_for(0, child)?;
        self.graph.add_child(parent, child)?;
        Ok(())
    }
    fn completed(
        &mut self,
        key: &IntegralKey,
        source: &CandidateReducer<N>,
        imported: bool,
    ) -> Result<(), CandidateGuardedCacheError> {
        let node = self.graph.node_for(0, key)?;
        self.graph.finish(node)?;
        if imported || key == self.root {
            return Ok(());
        }
        if !self.graph.is_active() {
            self.graph.record_skipped();
            return Ok(());
        }
        // Root-rank scope is a persisted-namespace boundary, not a bound on
        // native descendants. Out-of-entry descendants remain in the DAG but
        // cannot be exported into a falsely broader guarded-root namespace.
        match self.program.validate_entry(key, self.limits) {
            Ok(()) => {}
            Err(CandidateReductionError::OutsideNumeratorRank { .. }) => {
                self.graph.record_skipped();
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        }
        let cache_key = ApplicationCacheKey::guarded_root(key);
        if !self
            .sink
            .wants(&cache_key, self.program.context())
            .map_err(GuardedCacheError::from)?
        {
            self.graph.record_skipped();
            return Ok(());
        }
        if !self.graph.prepare_export(node)? {
            self.graph.record_skipped();
            return Ok(());
        }
        let terms = source
            .cache
            .get(key)
            .ok_or(crate::reduction::ReductionError::ReducerInvariant {
                detail: "completed candidate capture has no coefficient memo",
            })?
            .terms
            .materialize(self.program.context(), self.limits);
        let terms = match terms {
            Ok(terms) => terms,
            // A factorized intermediate need not be materialized by ordinary
            // reduction (its contributions can cancel later). Optional cache
            // export must not turn that extra operation into a solve failure.
            Err(CandidateReductionError::Application(
                crate::reduction::ReductionError::ExactAlgebra(
                    crate::algebra::ExactAlgebraError::ResourceLimit { .. }
                    | crate::algebra::ExactAlgebraError::ExponentLimit { .. }
                    | crate::algebra::ExactAlgebraError::ResourceCountOverflow { .. }
                    | crate::algebra::ExactAlgebraError::ExponentArithmeticOverflow { .. },
                ),
            )) => {
                self.graph.record_skipped();
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let record = ApplicationCacheRecord {
            key: cache_key,
            terms,
            nonzero_conditions: self.graph.conditions(node)?,
        };
        if let Err(error) = self.program.validate_completed_record(&record, self.limits) {
            if optional_export_limit(&error) {
                self.graph.record_skipped();
                return Ok(());
            }
            return Err(error);
        }
        self.sink
            .store_completed(&record, self.program.context())
            .map_err(GuardedCacheError::from)?;
        self.captured += 1;
        Ok(())
    }
    fn root_conditions(&self) -> Vec<Coefficient> {
        self.graph.all_conditions()
    }
}

fn optional_export_limit(error: &CandidateGuardedCacheError) -> bool {
    let reduction = match error {
        CandidateGuardedCacheError::Cache(GuardedCacheError::Reduction(error))
        | CandidateGuardedCacheError::Candidate(CandidateReductionError::Application(error)) => {
            error
        }
        _ => return false,
    };
    crate::reduction::cache_policy::is_cache_limit(reduction)
        || matches!(
            reduction,
            crate::reduction::ReductionError::ExactAlgebra(
                crate::algebra::ExactAlgebraError::ResourceLimit { .. }
                    | crate::algebra::ExactAlgebraError::ExponentLimit { .. }
                    | crate::algebra::ExactAlgebraError::ResourceCountOverflow { .. }
                    | crate::algebra::ExactAlgebraError::ExponentArithmeticOverflow { .. }
            )
        )
}
