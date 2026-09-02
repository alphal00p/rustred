//! Immutable publication of one exactly closed sector.
//!
//! A layer owns its executable cover and, through that seal, the exact
//! predecessor snapshot used to prove every proper-subsector image.  The
//! publication computes one bounded BLAKE3 content identity over the complete
//! executable/proof payload. Retained `Arc` identity, not the digest string,
//! remains proof authority; snapshots use the digest only for deterministic
//! structural identity and ordering.
//! Publication covers only the exact sector and ordering named by the cover;
//! symmetry aliases require a later authenticated transport boundary.

mod content;
mod model;

pub(super) use content::{try_build_owner_content_key, try_compare_owner_content_exact};
pub(crate) use model::{ClosedSectorLayer, ClosedSectorLayerContentId};
