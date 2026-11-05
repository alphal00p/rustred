//! Writing a p library and structural equivalence (owner: M2-T8b rr-package).
use std::path::Path;

use rustred::propagator::error::PError;
use rustred::propagator::library::PLibrary;

use super::assemble::AssembledLibrary;

pub fn write_p_library(_library: &AssembledLibrary, _directory: &Path) -> Result<(), PError> {
    Err(PError::NotImplemented("M2-T8b"))
}

pub fn equivalent_p_libraries(_a: &PLibrary, _b: &PLibrary) -> Result<bool, PError> {
    Err(PError::NotImplemented("M2-T8b"))
}
