//! Translation of complete, sealed source batches with optional count diagnostics.

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
