mod payload;
mod render;
mod structure;

use crate::application::error::AppError;

pub(super) use payload::{preflight_family_payload, preflight_generated_relations};
pub(super) use render::preflight_output_bound;
pub(super) use structure::preflight_derivation_structure;

pub(super) fn derivation_bound_overflow() -> AppError {
    AppError::limit("application derivation resource accounting overflowed usize".to_owned())
}
