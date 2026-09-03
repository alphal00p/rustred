use super::model::{AccumulatedDeltaId, EvaluationKey, RawCoeffRef};

/// One operation waiting on values produced by the iterative coefficient-DAG
/// traversal.  Keeping this schedule shared by modular evaluation and exact
/// fallback prevents the two semantics from drifting as new node kinds are
/// introduced.
#[derive(Clone, Copy, Debug)]
pub(super) enum UnaryOperation {
    Neg,
    Inv,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum BinaryOperation {
    Add,
    Mul,
}

/// Explicit left-to-right postorder state for one coefficient root.
///
/// `AfterLeft` is separate from `FinishBinary` so a singularity in the left
/// operand is observed before the right operand is even entered, matching the
/// former recursive evaluator's deterministic chronology.
#[derive(Clone, Copy, Debug)]
pub(super) enum PostorderFrame {
    Enter {
        reference: RawCoeffRef,
        inherited: AccumulatedDeltaId,
    },
    FinishUnary {
        key: EvaluationKey,
        operation: UnaryOperation,
    },
    AfterLeft {
        key: EvaluationKey,
        right: RawCoeffRef,
        inherited: AccumulatedDeltaId,
        operation: BinaryOperation,
    },
    FinishBinary {
        key: EvaluationKey,
        operation: BinaryOperation,
    },
}
