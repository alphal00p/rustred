use std::{collections::BTreeMap, sync::Arc};

use crate::algebra::{Coefficient, ExactAlgebraLimits, IndexedCoefficient, IndexedPolynomial};
use crate::family::{ContractionMomentum, ScalarProductCoordinate};
use crate::identity::{
    IndexShift, IntegralShift, RowId, SelectedTranslatedSourceBatch, TranslatedSourceRequest,
};

use super::polynomial::RawPolynomial;

/// Nominated geometry, not an owner domain or a target-selection policy.
#[derive(Clone, Debug)]
pub struct TangentSourceSpec {
    pub differentiated_loop: usize,
    pub protected_denominators: [usize; 2],
    pub contractions: [ContractionMomentum; 3],
    pub recenter: IntegralShift,
    /// Optional multiplication by one native affine scalar product.
    pub multiplier: Option<ScalarProductCoordinate>,
}

/// Structural preflight bounds plus authenticated coefficient limits.
///
/// Native coefficient swell/allocator scratch is not an RSS guarantee. Native
/// calls are admitted by conservative outer-polynomial term bounds and their
/// results revalidated; campaign memory admission remains a separate concern.
#[derive(Clone, Copy, Debug)]
pub struct TangentSourceLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_denominators: usize,
    pub max_polynomial_terms: usize,
    pub max_polynomial_degree: u16,
    pub max_exponent_entries: usize,
    pub max_term_operations: usize,
    pub max_selected_sources: usize,
    pub max_product_terms: usize,
    pub max_conditions: usize,
    pub max_exact_operations: usize,
    pub max_retained_coordinate_cells: usize,
}
impl Default for TangentSourceLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_denominators: 256,
            max_polynomial_terms: 100_000,
            max_polynomial_degree: 4,
            max_exponent_entries: 4_000_000,
            max_term_operations: 4_000_000,
            max_selected_sources: 65_536,
            max_product_terms: 1_000_000,
            max_conditions: 2_000_000,
            max_exact_operations: 8_000_000,
            max_retained_coordinate_cells: 16_000_000,
        }
    }
}

/// Read-only coefficients in denominator variables, in the plan's base field.
#[derive(Clone, Debug)]
pub struct TangentPolynomial {
    pub(super) raw: RawPolynomial,
}
impl TangentPolynomial {
    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }
    pub fn term_count(&self) -> usize {
        self.raw.nterms()
    }
    pub fn terms(&self) -> impl Iterator<Item = (&Coefficient, &[u16])> {
        self.raw.coefficients.iter().zip(self.raw.exponents_iter())
    }
}

/// One exact original row/translation and a base-field multiplier.
#[derive(Clone, Debug)]
pub struct WeightedTangentSource {
    pub(super) request: TranslatedSourceRequest,
    pub(super) row_id: RowId,
    pub(super) weight: Coefficient,
}
impl WeightedTangentSource {
    pub fn request(&self) -> &TranslatedSourceRequest {
        &self.request
    }
    pub fn row_id(&self) -> &RowId {
        &self.row_id
    }
    pub fn weight(&self) -> &Coefficient {
        &self.weight
    }
}

/// Sealed family-derived plan. There is no constructor from serialized weights.
#[derive(Debug)]
pub struct TangentSourcePlan {
    pub(super) family: Arc<String>,
    pub(super) arity: usize,
    pub(super) spec: TangentSourceSpec,
    pub(super) vector: [TangentPolynomial; 3],
    pub(super) contributions: Vec<WeightedTangentSource>,
    pub(super) limits: TangentSourceLimits,
}
impl TangentSourcePlan {
    pub fn family_fingerprint(&self) -> &str {
        &self.family
    }
    pub fn spec(&self) -> &TangentSourceSpec {
        &self.spec
    }
    pub fn vector_coefficients(&self) -> &[TangentPolynomial; 3] {
        &self.vector
    }
    /// Canonical offset-major/source-ordinal-minor order.
    pub fn contributions(&self) -> &[WeightedTangentSource] {
        &self.contributions
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TangentConditionOrigin {
    OriginalCondition { source: usize, condition: usize },
    OriginalCoefficientDenominator { source: usize, shift: IndexShift },
    WeightDenominator { source: usize },
}
#[derive(Clone, Debug)]
pub struct TangentSourceCondition {
    pub(super) polynomial: IndexedPolynomial,
    pub(super) origin: TangentConditionOrigin,
}
impl TangentSourceCondition {
    pub fn polynomial(&self) -> &IndexedPolynomial {
        &self.polynomial
    }
    pub fn origin(&self) -> &TangentConditionOrigin {
        &self.origin
    }
}

/// Complete indexed ordinary-source product, not a normalized or admitted rule.
///
/// The selected originals remain available, including coefficients that would
/// vanish on a later fixed face. Conditions are recorded before multiplication
/// and cancellation. No physical zero, symmetry, or target pruning is applied.
#[derive(Debug)]
pub struct TangentSourceCombination {
    pub(super) sources: SelectedTranslatedSourceBatch,
    pub(super) weights: Vec<IndexedCoefficient>,
    pub(super) product: BTreeMap<IndexShift, IndexedCoefficient>,
    pub(super) conditions: Vec<TangentSourceCondition>,
}
impl TangentSourceCombination {
    pub fn family_fingerprint(&self) -> &str {
        self.sources.family_fingerprint()
    }
    pub fn context_fingerprint(&self) -> &str {
        self.sources.context_fingerprint()
    }
    pub fn sources(&self) -> &SelectedTranslatedSourceBatch {
        &self.sources
    }
    /// Same canonical order as `sources().sources()`.
    pub fn weights(&self) -> &[IndexedCoefficient] {
        &self.weights
    }
    pub fn product(&self) -> &BTreeMap<IndexShift, IndexedCoefficient> {
        &self.product
    }
    pub fn conditions(&self) -> &[TangentSourceCondition] {
        &self.conditions
    }
}
