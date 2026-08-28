//! Historical concrete vacuum-family IBP and reduction engine.
//!
//! This engine is retained as lower-loop oracle evidence. Production RustRed
//! uses the topology-neutral `IntegralFamily` and parametric pipeline instead.

mod family;
mod ibp;
mod integral;
mod linear;
mod reduction;
mod tensor_family;

pub use family::{
    Denominator, FamilyConstructionLimits, FamilyError, PropagatorSign, ScalarProductExpansion,
    VacuumFamily,
};
pub use ibp::{IbpGenerationError, IbpGenerator, IbpIdentity};
pub use integral::Integral;
pub use linear::LinearCombination;
pub use reduction::{
    ReductionCacheError, ReductionCacheLimits, ReductionError, ReductionStats, ReductionTable,
    SeedConfig, SeedGenerationError, SeedGenerationLimits, SparseReducer, generate_seeds,
    try_generate_seeds, try_generate_seeds_with_limits,
};
pub use tensor_family::{
    DEFAULT_MAX_TENSOR_EXPANSION_OPERATIONS, DEFAULT_MAX_TENSOR_EXPANSION_TERMS, TensorFamilyError,
    TensorFamilyReducer, TensorIntegralReduction,
};
