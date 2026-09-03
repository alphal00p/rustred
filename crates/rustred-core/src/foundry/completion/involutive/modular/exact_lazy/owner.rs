use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::identity::CompletedIbpSourceRows;

use super::super::super::{OreActionIdentity, OreOrderingAdapter};
use super::super::model::DagOwner;
use super::{ExactLazyError, ExactLazyLimits};

#[derive(Debug)]
struct ExactLazyOwnerBinding {
    dag: DagOwner,
    context_fingerprint: Arc<String>,
    action: OreActionIdentity,
    source_module_owner: Arc<()>,
    source_count: usize,
    arity: usize,
    limits: ExactLazyLimits,
}

/// Opaque authority of one uncompacted exact-lazy coefficient generation.
///
/// In particular this binds the completed-source chronology transitively
/// through `OreActionIdentity`; equal-looking sectors constructed from a
/// different source module are foreign.
#[derive(Clone, Debug)]
pub(super) struct ExactLazyOwner(Arc<ExactLazyOwnerBinding>);

impl ExactLazyOwner {
    pub(super) fn fresh(
        dag: DagOwner,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        completed: &CompletedIbpSourceRows,
        limits: ExactLazyLimits,
    ) -> Self {
        Self(Arc::new(ExactLazyOwnerBinding {
            dag,
            context_fingerprint: context.fingerprint_owner(),
            action: ordering.identity().clone(),
            source_module_owner: completed.identity_owner(),
            source_count: completed.source_row_count(),
            arity: ordering.arity(),
            limits,
        }))
    }

    pub(super) fn belongs_to(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    pub(super) fn owns_dag(&self, dag: &DagOwner) -> bool {
        self.0.dag.belongs_to(dag)
    }

    pub(super) fn require_binding(
        &self,
        ordering: &OreOrderingAdapter,
        context: &IndexedCoefficientContext,
        limits: ExactLazyLimits,
    ) -> Result<(), ExactLazyError> {
        if !self.0.action.belongs_to(ordering.identity()) {
            return Err(ExactLazyError::WrongOreAction);
        }
        if self.0.arity != ordering.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy Ore ordering",
                expected: self.0.arity,
                actual: ordering.arity(),
            });
        }
        if !context.owns_fingerprint(&self.0.context_fingerprint) {
            return Err(ExactLazyError::WrongIndexedContext);
        }
        if context.index_count() != self.0.arity {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy indexed context",
                expected: self.0.arity,
                actual: context.index_count(),
            });
        }
        if limits != self.0.limits {
            return Err(ExactLazyError::WrongLimitsContract);
        }
        Ok(())
    }

    pub(super) fn require_ordering(
        &self,
        ordering: &OreOrderingAdapter,
    ) -> Result<(), ExactLazyError> {
        if !self.0.action.belongs_to(ordering.identity()) {
            return Err(ExactLazyError::WrongOreAction);
        }
        if self.0.arity != ordering.arity() {
            return Err(ExactLazyError::WrongArity {
                object: "exact-lazy Ore ordering",
                expected: self.0.arity,
                actual: ordering.arity(),
            });
        }
        Ok(())
    }

    pub(super) fn require_completed_source_module(
        &self,
        ordering: &OreOrderingAdapter,
        completed: &CompletedIbpSourceRows,
    ) -> Result<(), ExactLazyError> {
        self.require_ordering(ordering)?;
        if ordering.owns_completed_source_module(completed)
            && completed.owns_identity(&self.0.source_module_owner)
            && completed.source_row_count() == self.0.source_count
            && completed.context_fingerprint() == self.0.context_fingerprint.as_str()
        {
            Ok(())
        } else {
            Err(ExactLazyError::WrongSourceModule)
        }
    }

    pub(super) fn arity(&self) -> usize {
        self.0.arity
    }

    pub(super) fn limits(&self) -> ExactLazyLimits {
        self.0.limits
    }
}

impl PartialEq for ExactLazyOwner {
    fn eq(&self, other: &Self) -> bool {
        self.belongs_to(other)
    }
}

impl Eq for ExactLazyOwner {}
