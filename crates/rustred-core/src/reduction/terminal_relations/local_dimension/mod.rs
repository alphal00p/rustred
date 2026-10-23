//! Fixed-d exact consequences of localized dimensional pairs.
//!
//! Spectators remain conditional variables of one local integration direction.
//! Every published row is independently reproduced from finite ordinary IBPs
//! at the correctly shifted seeds. No shifted-dimensional runtime master is
//! admitted, and exhausting these selected local directions is not minimality.

mod codec;
mod geometry;
mod model;
mod sources;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::{TerminalEquation, TerminalRelationRow};
use crate::algebra::{Coefficient, ExactAlgebraLimits};
use crate::family::{IntegralFamily, IntegralKey};

pub use model::{
    LocalDimensionRecurrenceControl, LocalDimensionRecurrenceError,
    LocalDimensionRecurrenceOptions, LocalDimensionRecurrencePlan,
    LocalDimensionRecurrenceProgress, LocalDimensionRecurrenceStatistics,
};

use LocalDimensionRecurrenceError as Error;
use model::{WeightedSource, Witness};

pub(super) fn algebra(error: impl std::fmt::Display) -> Error {
    Error::Algebra(error.to_string())
}
