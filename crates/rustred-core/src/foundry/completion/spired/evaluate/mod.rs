//! Direct modular evaluation of translated ordinary sources.

mod backend;
mod buffer;
mod error;
mod evaluator;

pub(crate) use buffer::{
    ShiftedModularResidueBuffer, ShiftedModularSourceBuffer, ShiftedModularTerm,
    ShiftedModularTerms,
};
pub(crate) use error::DirectShiftedSourceError;
pub(crate) use evaluator::{
    DirectShiftedSourceCorpusCensus, DirectShiftedSourceEvaluator, DirectShiftedSourceLimits,
    ValidatedDirectShiftedSources,
};

#[cfg(test)]
mod tests;
