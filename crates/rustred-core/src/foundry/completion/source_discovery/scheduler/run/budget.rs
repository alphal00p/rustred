//! Aggregate and local outer-resource accounting.

use super::super::{
    ProbeLocalBudgetCause, ProbeLocalBudgetScope, ProbeLocalRunCensus, ProbeLocalSchedulerError,
    ProbeLocalSchedulerLimits,
};

const AGGREGATE_EPOCHS: &str = "probe-local aggregate fresh epochs";
const AGGREGATE_EPOCH_REQUEST_WORK: &str = "probe-local aggregate epoch request work";
pub(super) const AGGREGATE_MATERIALIZED_SOURCE_TERMS: &str =
    "probe-local aggregate materialized source terms";
const AGGREGATE_MODULAR_ENTRY_WORK: &str = "probe-local aggregate modular entry work";
const AGGREGATE_RESIDUAL_CANDIDATE_WORK: &str = "probe-local aggregate residual candidate work";
pub(super) const AGGREGATE_RESIDUAL_SOURCE_TERM_WORK: &str =
    "probe-local aggregate residual source-term work";
const AGGREGATE_PROSPECTIVE_CLASSIFICATION_WORK: &str =
    "probe-local aggregate prospective classification work";
const AGGREGATE_MERGE_REQUEST_WORK: &str = "probe-local aggregate merge request work";
pub(super) const ITERATION_RECORDS: &str = "probe-local retained iteration records";
const EXACT_ATTEMPTS: &str = "probe-local exact-lift attempts";

#[derive(Default)]
pub(super) struct RunBudget {
    epochs: usize,
    epoch_request_work: usize,
    materialized_source_terms: usize,
    modular_entry_work: usize,
    residual_candidate_work: usize,
    residual_source_term_work: usize,
    prospective_classification_reservation: usize,
    merge_request_work: usize,
    iteration_records: usize,
    exact_attempts: usize,
}

impl RunBudget {
    pub(super) fn try_admit_epoch(
        &mut self,
        request_count: usize,
        source_term_work: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let epochs = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCHS,
            self.epochs,
            1,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCHS,
            epochs,
            limits.max_aggregate_epochs,
        )?;
        let work = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCH_REQUEST_WORK,
            self.epoch_request_work,
            request_count,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_EPOCH_REQUEST_WORK,
            work,
            limits.max_aggregate_epoch_request_work,
        )?;
        let terms = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MATERIALIZED_SOURCE_TERMS,
            self.materialized_source_terms,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MATERIALIZED_SOURCE_TERMS,
            terms,
            limits.max_aggregate_materialized_source_terms,
        )?;
        self.epochs = epochs;
        self.epoch_request_work = work;
        self.materialized_source_terms = terms;
        Ok(())
    }

    pub(super) fn try_admit_modular_work(
        &mut self,
        physical_entries: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let work = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MODULAR_ENTRY_WORK,
            self.modular_entry_work,
            physical_entries,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MODULAR_ENTRY_WORK,
            work,
            limits.max_aggregate_modular_entry_work,
        )?;
        self.modular_entry_work = work;
        Ok(())
    }

    /// Reserve every term in the nominated residual source batch before the
    /// exhaustive census begins. The same exact term count is a conservative
    /// upper bound for the more expensive prospective classifier, which is
    /// actually run only for retained nonzero rows.
    pub(super) fn try_admit_residual_work(
        &mut self,
        candidate_work: usize,
        source_term_work: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let candidates = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_CANDIDATE_WORK,
            self.residual_candidate_work,
            candidate_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_CANDIDATE_WORK,
            candidates,
            limits.max_aggregate_residual_candidate_work,
        )?;
        let source_terms = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_SOURCE_TERM_WORK,
            self.residual_source_term_work,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_RESIDUAL_SOURCE_TERM_WORK,
            source_terms,
            limits.max_aggregate_residual_source_term_work,
        )?;
        let classifications = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_PROSPECTIVE_CLASSIFICATION_WORK,
            self.prospective_classification_reservation,
            source_term_work,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_PROSPECTIVE_CLASSIFICATION_WORK,
            classifications,
            limits.max_aggregate_prospective_classification_work,
        )?;
        self.residual_candidate_work = candidates;
        self.residual_source_term_work = source_terms;
        self.prospective_classification_reservation = classifications;
        Ok(())
    }

    pub(super) fn try_admit_merge_work(
        &mut self,
        existing_requests: usize,
        candidate_requests: usize,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let local = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MERGE_REQUEST_WORK,
            existing_requests,
            candidate_requests,
        )?;
        let work = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MERGE_REQUEST_WORK,
            self.merge_request_work,
            local,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            AGGREGATE_MERGE_REQUEST_WORK,
            work,
            limits.max_aggregate_merge_request_work,
        )?;
        self.merge_request_work = work;
        Ok(())
    }

    pub(super) fn try_admit_iteration_record(
        &mut self,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let records = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            ITERATION_RECORDS,
            self.iteration_records,
            1,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            ITERATION_RECORDS,
            records,
            limits.max_retained_iteration_records,
        )?;
        self.iteration_records = records;
        Ok(())
    }

    pub(super) fn try_admit_exact(
        &mut self,
        limits: ProbeLocalSchedulerLimits,
    ) -> Result<(), ProbeLocalBudgetCause> {
        let attempts = checked_budget_add(
            ProbeLocalBudgetScope::Aggregate,
            EXACT_ATTEMPTS,
            self.exact_attempts,
            1,
        )?;
        check_outer(
            ProbeLocalBudgetScope::Aggregate,
            EXACT_ATTEMPTS,
            attempts,
            limits.max_exact_lift_attempts,
        )?;
        self.exact_attempts = attempts;
        Ok(())
    }

    pub(super) const fn census(&self) -> ProbeLocalRunCensus {
        ProbeLocalRunCensus::new(
            self.epochs,
            self.epoch_request_work,
            self.materialized_source_terms,
            self.modular_entry_work,
            self.residual_candidate_work,
            self.residual_source_term_work,
            self.prospective_classification_reservation,
            self.merge_request_work,
            self.iteration_records,
            self.exact_attempts,
        )
    }
}

pub(super) fn checked_budget_add(
    scope: ProbeLocalBudgetScope,
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProbeLocalBudgetCause> {
    left.checked_add(right)
        .ok_or(ProbeLocalBudgetCause::CountOverflow { scope, resource })
}

pub(super) fn check_outer(
    scope: ProbeLocalBudgetScope,
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeLocalBudgetCause> {
    if requested > limit {
        Err(ProbeLocalBudgetCause::Outer {
            scope,
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ProbeLocalSchedulerError> {
    left.checked_add(right)
        .ok_or(ProbeLocalSchedulerError::ResourceCountOverflow { resource })
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ProbeLocalSchedulerError> {
    if requested > limit {
        Err(ProbeLocalSchedulerError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    resource: &'static str,
    capacity: usize,
) -> Result<Vec<T>, ProbeLocalSchedulerError> {
    let mut values = Vec::new();
    values.try_reserve_exact(capacity).map_err(|_| {
        ProbeLocalSchedulerError::AllocationFailure {
            resource,
            requested: capacity,
        }
    })?;
    Ok(values)
}
