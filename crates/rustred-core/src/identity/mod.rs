//! Typed parametric identity rows, sparse relations, and exceptional domains.

mod condition;
mod generator;
mod relation;
mod row;
mod tangent;

pub use condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition,
};
pub use generator::{
    CompletedIbpSourceRows, IbpSourceRow, IntegralShift, ParametricIbpConfig, ParametricIbpError,
    ParametricIbpGenerator, PreparedIbpSourceBatch, PreparedLorentzInvarianceBatch,
    SelectedTranslatedSourceBatch, TranslatedSource, TranslatedSourceBatch, TranslatedSourceError,
    TranslatedSourceLimits, TranslatedSourceProvenance, TranslatedSourceRequest,
};
pub use relation::{IndexShift, ParametricRelation, ParametricRelationError, RelationLimits};
pub use row::RowId;
pub use tangent::{
    TangentConditionOrigin, TangentPolynomial, TangentSourceCombination, TangentSourceCondition,
    TangentSourceError, TangentSourceLimits, TangentSourcePlan, TangentSourceSpec,
    WeightedTangentSource,
};

pub(crate) use relation::Builder as RelationBuilder;
