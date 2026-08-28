use crate::algebra::IndexedAlgebraLimits;

use super::super::condition::IdentityConditionLimits;

/// Complete arithmetic and identity-condition policy for relation operations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RelationLimits {
    pub arithmetic: IndexedAlgebraLimits,
    pub identity_conditions: IdentityConditionLimits,
}
