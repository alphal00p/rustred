//! Preserve native typed failure categories at the private backend boundary.
use crate::{AppError, AppErrorKind as K};
use rustred::{
    algebra::{ExactAlgebraError, IndexedAlgebraError},
    reduction::ReductionError,
    sector::symmetry::integral_transport::{Error as Transport, ExpansionError},
    solver::{
        CandidateReductionError, CandidateRoutedCampaignFailure,
        CandidateRoutedCompletedTraceError, CandidateRoutedError,
    },
};

pub(super) fn completed<const N: usize>(error: CandidateRoutedCompletedTraceError<N>) -> AppError {
    let kind = match &error {
        CandidateRoutedCompletedTraceError::Frontier(_) => K::Execution,
        CandidateRoutedCompletedTraceError::Retention { reason, .. } => routed(reason),
        CandidateRoutedCompletedTraceError::Trace(error) => match error.reason() {
            CandidateRoutedCampaignFailure::Cancelled => K::Cancelled,
            CandidateRoutedCampaignFailure::WorkerPanicked => K::Execution,
            CandidateRoutedCampaignFailure::Trace(error) => routed(error),
            CandidateRoutedCampaignFailure::Backing(error) => {
                super::storage_error(error.clone().into()).kind()
            }
        },
    };
    AppError::new(kind, error.to_string())
}
fn routed(error: &CandidateRoutedError) -> K {
    match error {
        CandidateRoutedError::ResourceLimit { .. } => K::Limit,
        CandidateRoutedError::InvalidInput(_) | CandidateRoutedError::EntryAdmission(_) => K::Input,
        CandidateRoutedError::Candidate(error) => match error {
            CandidateReductionError::TraceInputLimit { .. }
            | CandidateReductionError::TraceIntegralLimit { .. }
            | CandidateReductionError::IndexOverflow { .. } => K::Limit,
            CandidateReductionError::Algebra(error) if indexed_limit(error) => K::Limit,
            CandidateReductionError::Application(error) if reduction_limit(error) => K::Limit,
            _ => K::Execution,
        },
        CandidateRoutedError::Transport(Transport::Expansion(error)) if expansion_limit(error) => {
            K::Limit
        }
        _ => K::Execution,
    }
}
fn exact_limit(error: &ExactAlgebraError) -> bool {
    matches!(
        error,
        ExactAlgebraError::ExponentLimit { .. }
            | ExactAlgebraError::ExponentArithmeticOverflow { .. }
            | ExactAlgebraError::ResourceLimit { .. }
            | ExactAlgebraError::ResourceCountOverflow { .. }
    )
}
fn indexed_limit(error: &IndexedAlgebraError) -> bool {
    match error {
        IndexedAlgebraError::ResourceLimit { .. }
        | IndexedAlgebraError::ResourceCountOverflow { .. }
        | IndexedAlgebraError::AllocationFailure { .. } => true,
        IndexedAlgebraError::ExactAlgebra(error) => exact_limit(error),
        _ => false,
    }
}
fn reduction_limit(error: &ReductionError) -> bool {
    match error {
        ReductionError::RuleApplicationLimit { .. }
        | ReductionError::CoalescingAdditionLimit { .. }
        | ReductionError::AllocationFailure { .. }
        | ReductionError::CacheLimit { .. }
        | ReductionError::CacheCoefficientTermLimit { .. }
        | ReductionError::CacheCoefficientByteLimit { .. }
        | ReductionError::CacheResourceCountOverflow { .. }
        | ReductionError::PendingFrameLimit { .. }
        | ReductionError::FactorizationTermLimit { .. }
        | ReductionError::IndexOverflow { .. }
        | ReductionError::CommonMassPowerOverflow => true,
        ReductionError::ExactAlgebra(error) => exact_limit(error),
        ReductionError::IndexedAlgebra(error) => indexed_limit(error),
        _ => false,
    }
}
fn expansion_limit(error: &ExpansionError) -> bool {
    match error {
        ExpansionError::ResourceLimit { .. }
        | ExpansionError::ResourceCountOverflow { .. }
        | ExpansionError::AllocationFailure { .. }
        | ExpansionError::NativeExponentLimit { .. }
        | ExpansionError::NativePolynomialSupportExceeded { .. }
        | ExpansionError::NativeExponentDegreeOverflow { .. }
        | ExpansionError::PowerShiftUnderflow { .. } => true,
        ExpansionError::ExactAlgebra(error) => exact_limit(error),
        _ => false,
    }
}
