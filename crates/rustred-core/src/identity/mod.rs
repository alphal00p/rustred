//! Typed parametric identity rows, sparse relations, and exceptional domains.

mod condition;
mod generator;
mod relation;
mod row;

pub use condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition,
};
pub use generator::{
    CompletedIbpSourceRows, IbpSourceRow, IntegralShift, ParametricIbpConfig, ParametricIbpError,
    ParametricIbpGenerator, PreparedIbpSourceBatch, PreparedLorentzInvarianceBatch,
    TranslatedSource, TranslatedSourceBatch, TranslatedSourceError, TranslatedSourceLimits,
    TranslatedSourceProvenance,
};
pub use relation::{IndexShift, ParametricRelation, ParametricRelationError, RelationLimits};
pub use row::RowId;
