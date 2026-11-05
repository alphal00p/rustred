//! Custom-owner generation with the false-knock-out guard (owner: M2-T7c rr-custom).
use std::sync::Arc;

use rustred::propagator::error::PError;
use rustred::propagator::library::CustomOwnerMeta;
use rustred::propagator::presentation::Presentation;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomOwnerOptions {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CustomOwnerPayload {
    pub bytes: Arc<[u8]>,
    pub meta: CustomOwnerMeta,
}

pub fn generate_custom_owner(
    _presentation: &Presentation,
    _options: &CustomOwnerOptions,
) -> Result<CustomOwnerPayload, PError> {
    Err(PError::NotImplemented("M2-T7c"))
}
