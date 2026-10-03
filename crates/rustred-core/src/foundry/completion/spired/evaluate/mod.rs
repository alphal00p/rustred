//! Direct modular evaluation of translated ordinary sources.

mod backend;
mod buffer;
mod error;
mod evaluator;

pub(crate) use buffer::{
    ShiftedModularResidueBuffer, ShiftedModularSourceBuffer, ShiftedModularTerm,
    ShiftedModularTerms,
};
pub use error::DirectShiftedSourceError;
pub use evaluator::DirectShiftedSourceLimits;
pub(crate) use evaluator::{
    DirectShiftedSourceCorpusCensus, DirectShiftedSourceEvaluator, ValidatedDirectShiftedSources,
};

#[cfg(test)]
mod tests;
