//! Generated numerator cells on the factorized four-line K6 face.
//!
//! The scalar sector itself is an authenticated product. Negative powers on
//! its inactive edges remain genuine numerator obligations, so each certified
//! endpoint or ray has an explicit semantic owner below this module.

mod bridge_dot_numerator;
#[cfg(test)]
mod bridge_dot_numerator_tests;
mod inactive_numerator_endpoint;
#[cfg(test)]
mod inactive_numerator_endpoint_tests;
mod triangle_dot_numerator;
#[cfg(test)]
mod triangle_dot_numerator_tests;
mod two_dot_numerator_endpoint;
#[cfg(test)]
mod two_dot_numerator_endpoint_tests;

pub(super) use bridge_dot_numerator::derive_factorized_bridge_dot_numerator_cells;
pub(super) use inactive_numerator_endpoint::derive_factorized_face_numerator_endpoint;
pub(super) use triangle_dot_numerator::derive_factorized_triangle_dot_numerator_cells;
pub(super) use two_dot_numerator_endpoint::derive_factorized_two_dot_numerator_endpoint;

pub(super) const FACTORIZED_FACE_SECTOR: [i64; 6] = [0, 0, 1, 1, 1, 1];
