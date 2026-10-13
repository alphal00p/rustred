//! Provider-driven exact normalization of symbolic vacuum products.
//! Providers own existing rule application; this module owns typed product
//! algebra, strict lower-loop recursion, guards, budgets and memoization.
mod arithmetic;
mod engine;
mod model;
pub use model::{
    VacuumFactor, VacuumFactorProvider, VacuumFactorReduction, VacuumFactorTerm, VacuumFactorWork,
    VacuumProductCacheStatistics, VacuumProductReducer, VacuumProductReduction,
    VacuumProductReductionError, VacuumProductReductionLimits, VacuumProductReductionStatistics,
};

#[cfg(test)]
mod tests;
