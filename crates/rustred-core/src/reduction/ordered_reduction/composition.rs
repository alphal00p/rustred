//! Terminal composition of modular Ordered requests ("compose before lift").
//!
//! A caller whose terminals are not its final outputs (a saved application
//! substitutes every raw terminal by an exact row over its masters) can
//! install an [`OrderedTerminalComposer`]. A modular request (`ModularSampled`
//! or `ModularPolynomial`) then applies the caller's sparse linear map to the
//! terminal weights modulo every prime, before rational reconstruction:
//!
//! - `ModularSampled` composes the replayed terminal values at every sample
//!   point and interpolates only the composed outputs;
//! - `ModularPolynomial` composes the per-prime terminal images in `Z_p(d)`
//!   (by evaluation at fresh points of that prime and Thiele interpolation of
//!   the composed outputs).
//!
//! Only the composed outputs are lifted over Q, by the same driver
//! (signature majority, stability, fresh-prime verification) as raw
//! terminals. The map is exact: every coefficient is reduced modulo each
//! prime, a prime dividing a coefficient denominator identically is
//! unusable, and a sample point at a pole of a coefficient is a bad point.
//! The lifted outputs therefore equal the exact composition of the exact
//! terminal coefficients with the same rows (the result the caller computes
//! without a composer), while the large intermediate terminal coefficients
//! are never reconstructed.
use std::any::Any;
use std::sync::Arc;

use crate::algebra::Coefficient;
use crate::family::IntegralKey;
use crate::reduction::ReductionError;

/// One stage of a composition plan: for every input (the request's live
/// terminals for the first stage, the previous stage's outputs otherwise)
/// its nonzero entries `(output index, exact coefficient)`.
pub struct OrderedCompositionStage<'a> {
    pub rows: Vec<Vec<(u32, &'a Coefficient)>>,
    /// Number of outputs of this stage.
    pub width: usize,
}

/// The exact sparse map of one request's live terminals onto the caller's
/// outputs, borrowed from the composer for the duration of the request.
pub struct OrderedCompositionPlan<'a> {
    pub stages: Vec<OrderedCompositionStage<'a>>,
    /// Caller payload describing the final outputs (returned unchanged).
    pub labels: Arc<dyn Any + Send + Sync>,
}

impl OrderedCompositionPlan<'_> {
    /// Number of final outputs.
    pub fn outputs(&self) -> usize {
        self.stages.last().map_or(0, |stage| stage.width)
    }

    /// Structural checks against the request's live terminals.
    pub(crate) fn validate(&self, terminals: usize) -> Result<(), ReductionError> {
        let invalid = |detail: &str| ReductionError::ModularWeights {
            detail: format!("terminal composition plan: {detail}"),
        };
        let mut inputs = terminals;
        if self.stages.is_empty() {
            return Err(invalid("no stage"));
        }
        for stage in &self.stages {
            if stage.rows.len() != inputs {
                return Err(invalid("a stage has a wrong number of inputs"));
            }
            if u32::try_from(stage.width).is_err() {
                return Err(invalid("too many outputs"));
            }
            for row in &stage.rows {
                for (output, coefficient) in row {
                    if *output as usize >= stage.width {
                        return Err(invalid("an entry names an output beyond the stage width"));
                    }
                    if coefficient.is_zero() {
                        return Err(invalid("an entry has a zero coefficient"));
                    }
                }
            }
            inputs = stage.width;
        }
        Ok(())
    }
}

/// Supplies the composition of one modular Ordered request (see the module
/// docs). Installed by the caller of a routed candidate application.
pub trait OrderedTerminalComposer: Send + Sync {
    /// The plan for the live terminals of one request, in key order, or
    /// `None` to lift the terminals themselves (the caller then composes the
    /// exact terminal coefficients as without a composer).
    fn plan<'a>(
        &'a self,
        terminals: &[IntegralKey],
    ) -> Result<Option<OrderedCompositionPlan<'a>>, ReductionError>;
}

/// Lifted outputs of a composed modular request.
#[derive(Clone)]
pub struct ComposedTerminals {
    /// The live terminals of the request (inputs of the plan), in key order.
    pub terminals: Vec<IntegralKey>,
    /// The plan's labels.
    pub labels: Arc<dyn Any + Send + Sync>,
    /// Exact coefficient of every final output, zeros included.
    pub coefficients: Vec<Coefficient>,
}

impl std::fmt::Debug for ComposedTerminals {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ComposedTerminals")
            .field("terminals", &self.terminals.len())
            .field("coefficients", &self.coefficients)
            .finish()
    }
}

impl PartialEq for ComposedTerminals {
    fn eq(&self, other: &Self) -> bool {
        self.terminals == other.terminals && self.coefficients == other.coefficients
    }
}

impl Eq for ComposedTerminals {}

/// An optional installed composer (`Debug` without exposing the caller's
/// type).
#[derive(Clone, Default)]
pub struct OrderedTerminalComposerSlot(pub Option<Arc<dyn OrderedTerminalComposer>>);

impl std::fmt::Debug for OrderedTerminalComposerSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Some(OrderedTerminalComposer)"
        } else {
            "None"
        })
    }
}
