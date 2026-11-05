//! Library assembly for scope 1..max_loops (owner: M2-T8a rr-assemble).
use std::sync::Arc;

use rustred::propagator::error::PError;
use rustred::propagator::families::PFamiliesFile;
use rustred::propagator::library::PLibrary;

use super::CustomPayloads;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AssembleOptions {}

#[derive(Clone, Debug)]
pub struct AssembledLibrary {
    pub library: Arc<PLibrary>,
}

pub fn assemble_library(
    _families: &PFamiliesFile,
    _max_loops: u8,
    _custom: CustomPayloads,
    _options: &AssembleOptions,
) -> Result<AssembledLibrary, PError> {
    Err(PError::NotImplemented("M2-T8a"))
}
