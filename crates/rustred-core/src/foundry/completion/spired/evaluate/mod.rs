//! Direct modular evaluation of translated ordinary sources.

mod backend;
mod buffer;
mod error;
mod evaluator;

pub(crate) use buffer::{ShiftedModularSourceBuffer, ShiftedModularTerm, ShiftedModularTerms};
pub(crate) use error::DirectShiftedSourceError;
pub(crate) use evaluator::{DirectShiftedSourceEvaluator, DirectShiftedSourceLimits};

#[cfg(test)]
mod tests;
