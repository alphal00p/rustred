use std::collections::BTreeMap;

use symbolica::prelude::{Atom, Symbol};

use crate::algebra::{Coefficient, CoefficientContext};
use crate::family::{AffineDenominator, ScalarProductCoordinate};

use super::limits::SymbolicaAffineDenominatorLimits;

/// Retained source, canonical normalized expression, and authenticated affine row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledSymbolicaAffineDenominator {
    pub(super) source: Atom,
    pub(super) normalized_expression: Atom,
    pub(super) affine_denominator: AffineDenominator,
}

impl CompiledSymbolicaAffineDenominator {
    pub const fn source(&self) -> &Atom {
        &self.source
    }

    pub const fn normalized_expression(&self) -> &Atom {
        &self.normalized_expression
    }

    pub const fn affine_denominator(&self) -> &AffineDenominator {
        &self.affine_denominator
    }
}

/// One reusable, topology-neutral denominator-expression compiler.
#[derive(Debug)]
pub(in crate::input) struct SymbolicaAffineDenominatorCompiler {
    pub(super) coefficients: CoefficientContext,
    pub(super) loop_momenta: Vec<String>,
    pub(super) external_momenta: Vec<String>,
    pub(super) external_gram: Vec<Vec<Coefficient>>,
    pub(super) combined: CoefficientContext,
    pub(super) symbol_positions: BTreeMap<Symbol, usize>,
    pub(super) scalar_product: Symbol,
    pub(super) coordinates: Vec<ScalarProductCoordinate>,
    pub(super) limits: SymbolicaAffineDenominatorLimits,
}
