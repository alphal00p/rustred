use crate::foundry::cell::SourceViewBatch;
use crate::foundry::parametric::ParametricRule;

/// Unforgeable crate-internal capability for the raw-part constructors used
/// by this lowering boundary. The type is nameable by sibling modules, but
/// only this module tree can mint a value.
#[derive(Debug)]
pub(crate) struct ExactCircuitLoweringSeal(());

impl ExactCircuitLoweringSeal {
    pub(super) const fn new() -> Self {
        Self(())
    }
}

/// A losslessly lowered exact identity. This value carries no application or
/// completion authority.
#[derive(Debug)]
pub(crate) struct LoweredExactCircuit {
    rule: ParametricRule,
    sources: SourceViewBatch,
}

impl LoweredExactCircuit {
    pub(crate) const fn rule(&self) -> &ParametricRule {
        &self.rule
    }

    pub(crate) const fn sources(&self) -> &SourceViewBatch {
        &self.sources
    }

    /// Transfer the replayed identity into the rule-cell admission boundary.
    /// The pair remains incapable of constructing a cell without that
    /// boundary's independent domain, descent, and guard checks.
    pub(crate) fn into_parts(self) -> (ParametricRule, SourceViewBatch) {
        (self.rule, self.sources)
    }

    pub(super) const fn new(rule: ParametricRule, sources: SourceViewBatch) -> Self {
        Self { rule, sources }
    }
}
