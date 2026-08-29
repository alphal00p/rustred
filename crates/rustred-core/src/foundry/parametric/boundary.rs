//! Parametric wrappers around topology-neutral sector-monotone proofs.

mod build;
mod model;

pub(super) use build::{build_sector_monotone_admission, preflight_sector_monotone_rhs_shift};
pub use model::{
    SectorMonotoneDependency, SectorMonotoneDependencyAtPoint, SectorMonotoneDependencyKind,
    SectorMonotoneTargetAdmission,
};
