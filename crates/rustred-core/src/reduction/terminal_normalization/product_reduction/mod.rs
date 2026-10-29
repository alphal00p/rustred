//! Provider-driven exact normalization of symbolic vacuum products.
//! Providers own existing rule application; this module owns typed product
//! algebra, strict lower-loop recursion, guards, budgets and memoization.
mod arithmetic;
mod candidate_batch;
mod combination;
mod engine;
mod model;
mod without_memoization;
mod zero;
pub use candidate_batch::reduce_transported_candidate_factor;
pub use combination::{
    reduce_transported_vacuum_factor, reduce_transported_vacuum_factor_without_memoization,
    reduce_vacuum_factor_combination, reduce_vacuum_factor_combination_without_memoization,
};
pub use model::{
    VacuumFactor, VacuumFactorProvider, VacuumFactorReduction, VacuumFactorTerm, VacuumFactorWork,
    VacuumProductCacheStatistics, VacuumProductReducer, VacuumProductReduction,
    VacuumProductReductionError, VacuumProductReductionLimits, VacuumProductReductionStatistics,
};

#[cfg(test)]
mod tests;
