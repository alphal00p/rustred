//! Deterministic bounded searches on one exact-sector integral lattice.

mod build;
mod error;
mod limits;
mod model;

pub use error::SectorSearchError;
pub use limits::SectorSearchLimits;
pub use model::SectorSearchDiamond;

#[cfg(test)]
mod tests;
