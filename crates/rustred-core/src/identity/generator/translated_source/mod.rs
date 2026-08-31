//! Bounded translation of complete, sealed parametric source batches.

mod construction;
mod error;
mod limits;
mod model;
mod selected;

pub use error::TranslatedSourceError;
pub use limits::TranslatedSourceLimits;
pub use model::{
    IntegralShift, SelectedTranslatedSourceBatch, TranslatedSource, TranslatedSourceBatch,
    TranslatedSourceProvenance, TranslatedSourceRequest,
};

#[cfg(test)]
mod tests;
