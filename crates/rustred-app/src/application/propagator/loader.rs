//! The fail-closed p-library loader (owner: M2-T8b rr-package).
use std::sync::Arc;

use rustred::propagator::error::PError;
use rustred::propagator::library::PLibrary;

use super::{PLibraryOptions, PLibrarySource};

pub fn load_p_library(
    _source: PLibrarySource,
    _options: &PLibraryOptions,
) -> Result<Arc<PLibrary>, PError> {
    Err(PError::NotImplemented("M2-T8b"))
}
