use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::AtomicUsize;

use symbolica::prelude::PolyVariable;

use crate::algebra::{Coefficient, CoefficientContext};

use super::super::value::IndexedCoefficient;

/// One exact pair of authenticated fields `K` and `K(n)`.
#[derive(Clone, Debug)]
pub struct IndexedCoefficientContext {
    pub(in crate::algebra::indexed) base: CoefficientContext,
    // Moving the fallibly constructed String into Arc only allocates the
    // fixed-size control block; unlike String -> Arc<str>, it does not make
    // another caller-sized allocation and copy.
    pub(in crate::algebra::indexed) fingerprint: Arc<String>,
    pub(in crate::algebra::indexed) variables: Arc<Vec<PolyVariable>>,
    pub(in crate::algebra::indexed) index_variables: Arc<Vec<PolyVariable>>,
    pub(in crate::algebra::indexed) template: Coefficient,
    #[cfg(test)]
    pub(super) authentication_counters: Arc<AuthenticationCounters>,
}

/// A coefficient borrowed after it has been bound to one exact indexed
/// context identity.
///
/// Fields are private to the context module so only
/// [`IndexedCoefficientContext`] can mint this proof. Relation assembly can
/// deliberately choose between a full ingress authentication and a
/// pointer-only binding of a value already sealed inside a relation.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundIndexedCoefficient<'context, 'value> {
    pub(super) value: &'value IndexedCoefficient,
    pub(super) bound_context: &'context Arc<String>,
}

#[cfg(test)]
#[derive(Debug, Default)]
pub(super) struct AuthenticationCounters {
    pub(super) full_operand_scans: AtomicUsize,
    pub(super) authenticated_native_results: AtomicUsize,
}
