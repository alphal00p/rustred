//! Cold compilation of exact inactive-numerator routing at a factorization.
//!
//! A certified unimodular factorization basis can turn all but one parent
//! denominator into canonical denominator coordinates.  This module derives
//! that routing through Symbolica and compiles the remaining affine form into
//! a constant-width, one-factor-at-a-time auxiliary recurrence.  It owns no
//! closing-artifact schema entry and is not dispatched by the reducer yet.

mod compile;
mod error;
mod limits;
mod model;
mod recurrence;

pub(crate) use compile::compile_factorized_numerator_lift;
pub(crate) use error::FactorizedNumeratorLiftError;
pub(crate) use limits::FactorizedNumeratorLiftLimits;
pub(crate) use model::{
    CompiledFactorizationRouting, FactorizedNumeratorLiftAction,
    FactorizedNumeratorLiftCompilation, FactorizedNumeratorLiftStart,
    FactorizedNumeratorLiftUnsupportedReason, RoutedAffineDenominator,
};

#[cfg(test)]
mod tests;
