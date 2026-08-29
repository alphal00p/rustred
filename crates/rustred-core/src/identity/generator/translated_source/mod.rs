//! Bounded translation of complete, sealed parametric source batches.

mod construction;
mod error;
mod limits;
mod model;

pub use error::TranslatedSourceError;
pub use limits::TranslatedSourceLimits;
pub use model::{
    IntegralShift, TranslatedSource, TranslatedSourceBatch, TranslatedSourceProvenance,
};

#[cfg(test)]
mod tests;
