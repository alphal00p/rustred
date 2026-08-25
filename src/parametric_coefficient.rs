//! Authenticated exact coefficient fields for parametric integral identities.
//!
//! A family is defined over a base field `K = Q(theta)`.  Parametric IBP
//! coefficients live in the strictly extended field `K(n)`, whose index
//! variables are internal RustRed symbols appended after every base variable.
//! Symbolica can automatically unify variable maps; this module deliberately
//! rejects that behavior at the proof-bearing boundary.

use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::mem::{align_of, size_of};
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::backend::integer::write_lsf_bytes;
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::GuardOrigin;
use crate::coefficient::{
    ExactAlgebraError, ExactAlgebraLimits, checked_coefficient_add_on_map,
    checked_coefficient_div_on_map, checked_coefficient_mul_on_map, checked_coefficient_neg_on_map,
    checked_coefficient_sub_on_map, checked_polynomial_mul_on_map, validate_coefficient_on_map,
    validate_polynomial_on_map,
};
use crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshPlanAuthorization;
use crate::{
    Coefficient, CoefficientContext, IndexShift, ResidualAffineIntegerMap,
    ResidualAffineIntegerSystemCertificate, ResidualAffineIntegerSystemError,
    ResidualUnitAffineIndexMapCertificate, ResidualUnitAffineIndexMapError,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

pub type CoefficientPolynomial = MultivariatePolynomial<IntegerRing, u16>;

/// One component of a pure index translation `n_i -> n_i + a_i`.
///
/// The trait is private so every exact boundary remains controlled by this
/// module.  In particular, Symbolica's public `Integer` variants have
/// representation-sensitive `Eq`/`Hash`, and `is_zero` recognizes only the
/// canonical `Single(0)`.  Exact components are therefore inspected
/// numerically and canonicalized only after the complete translation
/// preflight succeeds.
trait ParametricTranslationComponent {
    fn is_numeric_zero(&self) -> bool;
    fn magnitude_bits(&self) -> u128;
    fn to_canonical_integer(&self) -> Integer;
}

impl ParametricTranslationComponent for i64 {
    fn is_numeric_zero(&self) -> bool {
        *self == 0
    }

    fn magnitude_bits(&self) -> u128 {
        u128::from(i64::BITS - self.unsigned_abs().leading_zeros())
    }

    fn to_canonical_integer(&self) -> Integer {
        Integer::from(*self)
    }
}

impl ParametricTranslationComponent for Integer {
    fn is_numeric_zero(&self) -> bool {
        self.cmp(&Integer::Single(0)) == Ordering::Equal
    }

    fn magnitude_bits(&self) -> u128 {
        integer_magnitude_bits(self)
    }

    fn to_canonical_integer(&self) -> Integer {
        match self {
            Integer::Single(value) => Integer::from(*value),
            Integer::Double(value) => Integer::from(*value),
            // Arithmetic with canonical zero both canonicalizes a malformed
            // small `Large` and avoids inheriting an adversarially oversized
            // GMP capacity for a genuine large value.
            Integer::Large(_) => self + &Integer::Single(0),
        }
    }
}

/// A canonical coefficient known to belong to one exact `K(n)` variable map.
///
/// All public constructors normalize numerator and denominator to coprime
/// factors. This invariant lets integral index translations avoid a second
/// polynomial GCD: `n -> n + a` is a polynomial-ring automorphism and thus
/// preserves coprimality.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricCoefficient {
    raw: Coefficient,
    context: Arc<str>,
}

impl ParametricCoefficient {
    pub fn raw(&self) -> &Coefficient {
        &self.raw
    }

    #[cfg(test)]
    pub(crate) fn overwrite_numerator_exponent_for_test(&mut self, offset: usize, exponent: u16) {
        self.raw.numerator.exponents[offset] = exponent;
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }

    /// Conservative bytes owned by a deep clone of this authenticated
    /// rational coefficient. Shared variable maps and context fingerprints
    /// remain `Arc` seams and are deliberately not charged as deep payload.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>()
            .checked_add(polynomial_owned_retained_byte_bound(&self.raw.numerator)?)?
            .checked_add(polynomial_owned_retained_byte_bound(&self.raw.denominator)?)
    }

    /// Fallibly copy both owned sparse halves of an already authenticated
    /// rational coefficient while retaining its exact context identity.
    ///
    /// Callers must census and admit the complete sparse payload before this
    /// seam. Both user-sized backing vectors in each polynomial are reserved
    /// before their GMP coefficients are cloned. The authenticated variable
    /// map and RustRed context fingerprint remain shared `Arc` allocations;
    /// this deliberately performs no second normalization or polynomial GCD.
    pub(crate) fn try_copy_authenticated_sparse_payload(&self) -> Result<Self, &'static str> {
        Ok(Self {
            raw: RationalPolynomial {
                numerator: try_copy_authenticated_sparse_polynomial_payload(&self.raw.numerator)?,
                denominator: try_copy_authenticated_sparse_polynomial_payload(
                    &self.raw.denominator,
                )?,
            },
            context: self.context.clone(),
        })
    }

    /// Fallibly copy the numerator condition of a coefficient that the caller
    /// has already authenticated and censused against its exact `K(n)` map.
    pub(crate) fn try_copy_prevalidated_numerator_condition(
        &self,
    ) -> Result<ParametricPolynomial, &'static str> {
        Ok(ParametricPolynomial {
            raw: try_copy_authenticated_sparse_polynomial_payload(&self.raw.numerator)?,
            context: self.context.clone(),
        })
    }

    /// Fallibly copy the denominator condition of a coefficient that the
    /// caller has already authenticated and censused against its exact map.
    pub(crate) fn try_copy_prevalidated_denominator_condition(
        &self,
    ) -> Result<ParametricPolynomial, &'static str> {
        Ok(ParametricPolynomial {
            raw: try_copy_authenticated_sparse_polynomial_payload(&self.raw.denominator)?,
            context: self.context.clone(),
        })
    }
}

/// Allocation-free census of one authenticated rational coefficient's
/// complete sparse validation payload.  Outer replay certificates use this
/// to enforce a single aggregate row budget instead of resetting a nested
/// exact-algebra allowance for every coefficient.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricCoefficientValidationPayloadCensus {
    source_terms: usize,
    source_exponent_entries: usize,
    source_integer_bits: usize,
}

impl ParametricCoefficientValidationPayloadCensus {
    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn source_exponent_entries(self) -> usize {
        self.source_exponent_entries
    }

    pub(crate) const fn source_integer_bits(self) -> usize {
        self.source_integer_bits
    }
}

/// Allocation-free census of one authenticated polynomial's complete sparse
/// validation payload. It shares the same unit definitions as the rational
/// coefficient census so an outer private-row compiler can debit guards and
/// coefficient halves from one aggregate allowance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricPolynomialValidationPayloadCensus {
    source_terms: usize,
    source_exponent_entries: usize,
    source_integer_bits: usize,
}

impl ParametricPolynomialValidationPayloadCensus {
    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn source_exponent_entries(self) -> usize {
        self.source_exponent_entries
    }

    pub(crate) const fn source_integer_bits(self) -> usize {
        self.source_integer_bits
    }
}

/// Aggregate limits for one instrumented coefficient-field associate proof.
///
/// The proof compares the coefficient vectors over `Z[theta]` directly.  It
/// never constructs a rational function and never enters Symbolica's
/// polynomial normalization or GCD machinery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricPolynomialAssociateLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_context_fingerprint_comparison_bytes: usize,
    pub max_variable_map_entry_comparisons: usize,
    pub max_validation_terms: usize,
    pub max_validation_exponent_entries: usize,
    pub max_validation_integer_bits: usize,
    pub max_magnitude_copy_terms: usize,
    pub max_magnitude_copy_bytes: usize,
    pub max_term_order_entries: usize,
    pub max_index_groups: usize,
    pub max_ordering_comparison_bound: usize,
    pub max_index_coordinate_inspection_bound: usize,
    pub max_base_sum_coordinate_inspection_bound: usize,
    pub max_base_exponent_addition_bound: usize,
    pub max_anchor_cost_operations: usize,
    pub max_cross_terms: usize,
    pub max_peak_cross_terms: usize,
    pub max_integer_preflight_pairs: usize,
    pub max_integer_multiplication_bit_work_bound: usize,
    pub max_integer_accumulation_bit_work_bound: usize,
    pub max_integer_multiply_limb_operation_bound: usize,
    pub max_integer_accumulate_limb_operation_bound: usize,
    pub max_scratch_limb_write_bound: usize,
    pub max_product_scratch_limbs: usize,
    pub max_accumulator_scratch_limbs: usize,
    pub max_visible_temporary_byte_envelope: usize,
}

impl Default for ParametricPolynomialAssociateLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_context_fingerprint_comparison_bytes: usize::MAX,
            max_variable_map_entry_comparisons: usize::MAX,
            max_validation_terms: usize::MAX,
            max_validation_exponent_entries: usize::MAX,
            max_validation_integer_bits: usize::MAX,
            max_magnitude_copy_terms: usize::MAX,
            max_magnitude_copy_bytes: usize::MAX,
            max_term_order_entries: usize::MAX,
            max_index_groups: usize::MAX,
            max_ordering_comparison_bound: usize::MAX,
            max_index_coordinate_inspection_bound: usize::MAX,
            max_base_sum_coordinate_inspection_bound: usize::MAX,
            max_base_exponent_addition_bound: usize::MAX,
            max_anchor_cost_operations: usize::MAX,
            max_cross_terms: usize::MAX,
            max_peak_cross_terms: usize::MAX,
            max_integer_preflight_pairs: usize::MAX,
            max_integer_multiplication_bit_work_bound: usize::MAX,
            max_integer_accumulation_bit_work_bound: usize::MAX,
            max_integer_multiply_limb_operation_bound: usize::MAX,
            max_integer_accumulate_limb_operation_bound: usize::MAX,
            max_scratch_limb_write_bound: usize::MAX,
            max_product_scratch_limbs: usize::MAX,
            max_accumulator_scratch_limbs: usize::MAX,
            max_visible_temporary_byte_envelope: usize::MAX,
        }
    }
}

/// Measured and conservatively preflighted work for one associate proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricPolynomialAssociateStats {
    context_fingerprint_comparison_bytes: usize,
    variable_map_entry_comparisons: usize,
    validation_terms: usize,
    validation_exponent_entries: usize,
    validation_integer_bits: usize,
    magnitude_copy_terms: usize,
    magnitude_copy_bytes: usize,
    term_order_entries: usize,
    index_groups: usize,
    ordering_comparison_bound: usize,
    index_coordinate_inspection_bound: usize,
    base_sum_coordinate_inspection_bound: usize,
    base_exponent_addition_bound: usize,
    anchor_cost_operations: usize,
    cross_terms: usize,
    peak_cross_terms: usize,
    integer_preflight_pairs: usize,
    integer_multiplication_bit_work_bound: usize,
    integer_accumulation_bit_work_bound: usize,
    integer_multiply_limb_operation_bound: usize,
    integer_accumulate_limb_operation_bound: usize,
    scratch_limb_write_bound: usize,
    product_scratch_limbs: usize,
    accumulator_scratch_limbs: usize,
    visible_temporary_byte_envelope: usize,
    visible_temporary_bytes: usize,
}

macro_rules! parametric_polynomial_associate_stats_getters {
    ($($field:ident),+ $(,)?) => {$ (
        pub(crate) const fn $field(self) -> usize { self.$field }
    )+ };
}

impl ParametricPolynomialAssociateStats {
    parametric_polynomial_associate_stats_getters!(
        context_fingerprint_comparison_bytes,
        variable_map_entry_comparisons,
        validation_terms,
        validation_exponent_entries,
        validation_integer_bits,
        magnitude_copy_terms,
        magnitude_copy_bytes,
        term_order_entries,
        index_groups,
        ordering_comparison_bound,
        index_coordinate_inspection_bound,
        base_sum_coordinate_inspection_bound,
        base_exponent_addition_bound,
        anchor_cost_operations,
        cross_terms,
        peak_cross_terms,
        integer_preflight_pairs,
        integer_multiplication_bit_work_bound,
        integer_accumulation_bit_work_bound,
        integer_multiply_limb_operation_bound,
        integer_accumulate_limb_operation_bound,
        scratch_limb_write_bound,
        product_scratch_limbs,
        accumulator_scratch_limbs,
        visible_temporary_byte_envelope,
        visible_temporary_bytes,
    );
}

/// Result of one fully instrumented `K = Q(theta)` associate proof.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ParametricPolynomialAssociateResult {
    associated: bool,
    stats: ParametricPolynomialAssociateStats,
}

impl ParametricPolynomialAssociateResult {
    pub(crate) const fn associated(self) -> bool {
        self.associated
    }

    pub(crate) const fn stats(self) -> ParametricPolynomialAssociateStats {
        self.stats
    }
}

/// A polynomial over `K`'s integer polynomial ring, authenticated by its
/// ordered base variable map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BasePolynomial {
    raw: CoefficientPolynomial,
    context: Arc<str>,
}

impl BasePolynomial {
    /// Authenticate a base-field polynomial against an exact coefficient
    /// context. This is used when a later concrete quotient introduces a new
    /// nonzero condition that did not exist in the parametric source rows.
    pub fn try_from_raw(
        raw: CoefficientPolynomial,
        context: &CoefficientContext,
        limits: ExactAlgebraLimits,
    ) -> Result<Self, ParametricCoefficientError> {
        validate_polynomial_on_map(
            &raw,
            context.variables(),
            crate::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        Ok(Self {
            raw,
            context: base_context_fingerprint(context).into(),
        })
    }

    pub fn raw(&self) -> &CoefficientPolynomial {
        &self.raw
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.raw.is_one()
    }

    pub fn is_nonzero_constant(&self) -> bool {
        self.raw.is_constant() && !self.raw.is_zero()
    }

    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>().checked_add(polynomial_owned_retained_byte_bound(&self.raw)?)
    }
}

/// A polynomial over the exact index-extended map `K(n)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricPolynomial {
    raw: CoefficientPolynomial,
    context: Arc<str>,
}

/// Canonical sparse equality locus for a partial symbolic specialization.
/// Positions are sorted increasingly and unique, and the original index
/// arity is retained so the transcript cannot be replayed in another lattice.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PartialIndexAssignment {
    arity: usize,
    entries: Box<[(usize, i64)]>,
}

impl PartialIndexAssignment {
    pub fn try_new(
        entries: impl IntoIterator<Item = (usize, i64)>,
        arity: usize,
        max_assignments: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        let mut collected = Vec::new();
        for (ordinal, entry) in entries.into_iter().enumerate() {
            let requested = ordinal.checked_add(1).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "partial index assignments",
                },
            )?;
            check_limit("partial index assignments", requested, max_assignments)?;
            if entry.0 >= arity {
                return Err(ParametricCoefficientError::IndexAssignmentOutOfRange {
                    position: entry.0,
                    arity,
                });
            }
            collected.push(entry);
        }
        collected.sort_unstable_by_key(|&(position, _)| position);
        for pair in collected.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(ParametricCoefficientError::DuplicateIndexAssignment {
                    position: pair[0].0,
                });
            }
        }
        Ok(Self {
            arity,
            entries: collected.into_boxed_slice(),
        })
    }

    pub fn arity(&self) -> usize {
        self.arity
    }

    pub fn entries(&self) -> &[(usize, i64)] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn provenance_origin(&self) -> GuardOrigin {
        GuardOrigin::PartialIndexSpecialization {
            assignments: self.entries.clone(),
        }
    }
}

/// One authenticated polynomial nonzero condition with every atomic reason
/// it entered the exceptional-domain set.
///
/// Origins are stored in a `BTreeSet`, so merging the same polynomial is
/// deterministic and independent of relation assembly order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParametricNonZeroCondition {
    polynomial: ParametricPolynomial,
    origins: BTreeSet<GuardOrigin>,
}

impl ParametricNonZeroCondition {
    pub fn polynomial(&self) -> &ParametricPolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }

    /// Conservative bytes owned by a deep clone, including the sparse
    /// polynomial/GMP payload and every provenance node/owned atom payload.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        let mut bytes =
            size_of::<Self>().checked_add(self.polynomial.owned_retained_byte_bound()?)?;
        for origin in &self.origins {
            bytes = bytes.checked_add(origin.retained_byte_bound()?)?;
        }
        Some(bytes)
    }

    /// Attach an origin under an explicit provenance-cardinality budget.
    pub fn try_with_origin(
        mut self,
        origin: GuardOrigin,
        max_guard_origins: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        self.add_origin_with_limit(origin, max_guard_origins)?;
        Ok(self)
    }

    pub(crate) fn add_origin_with_limit(
        &mut self,
        origin: GuardOrigin,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        if !self.origins.contains(&origin) {
            let requested = self.origins.len().checked_add(1).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric guard origins",
                },
            )?;
            check_limit("parametric guard origins", requested, max_guard_origins)?;
            self.origins.insert(origin);
        }
        Ok(())
    }

    pub(crate) fn merge_origins_from(
        &mut self,
        other: &Self,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        debug_assert_eq!(self.polynomial, other.polynomial);
        let additional = other
            .origins
            .iter()
            .filter(|origin| !self.origins.contains(*origin))
            .count();
        let requested = self.origins.len().checked_add(additional).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric guard origins",
            },
        )?;
        check_limit("parametric guard origins", requested, max_guard_origins)?;
        self.origins.extend(other.origins.iter().cloned());
        Ok(())
    }
}

/// A specialized base-field polynomial condition with retained parametric
/// provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecializedNonZeroCondition {
    polynomial: BasePolynomial,
    origins: BTreeSet<GuardOrigin>,
}

impl SpecializedNonZeroCondition {
    pub fn from_base_polynomial(
        polynomial: BasePolynomial,
        origins: impl IntoIterator<Item = GuardOrigin>,
        max_guard_origins: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        if polynomial.is_zero() {
            return Err(ParametricCoefficientError::ZeroPolynomialCondition);
        }
        let origins = origins.into_iter().collect::<BTreeSet<_>>();
        if origins.is_empty() {
            return Err(ParametricCoefficientError::MissingGuardOrigin);
        }
        check_limit(
            "specialized guard origins",
            origins.len(),
            max_guard_origins,
        )?;
        Ok(Self {
            polynomial,
            origins,
        })
    }

    pub fn polynomial(&self) -> &BasePolynomial {
        &self.polynomial
    }

    pub fn origins(&self) -> &BTreeSet<GuardOrigin> {
        &self.origins
    }

    /// Conservative bytes owned by a deep clone, including the base-field
    /// sparse polynomial/GMP payload and every provenance atom.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        let mut bytes =
            size_of::<Self>().checked_add(self.polynomial.owned_retained_byte_bound()?)?;
        for origin in &self.origins {
            bytes = bytes.checked_add(origin.retained_byte_bound()?)?;
        }
        Some(bytes)
    }

    /// Replace replay-private generated-affine provenance with its public
    /// marker while leaving the exact nonzero polynomial untouched.
    ///
    /// This is intentionally crate-private: only a proof-bearing owner that
    /// retains the complete affine certificate may authorize the seal.  The
    /// marker is deterministic, so replay produces the same public concrete
    /// payload without publishing recentering vectors, shifts, row labels, or
    /// certificate-local coordinates.
    pub(crate) fn seal_generated_affine_provenance(&mut self) {
        if self.origins.len() == 1
            && self
                .origins
                .contains(&GuardOrigin::GeneratedAffineSealedCondition)
        {
            return;
        }
        self.origins.clear();
        self.origins
            .insert(GuardOrigin::GeneratedAffineSealedCondition);
    }

    pub(crate) fn add_origin_with_limit(
        &mut self,
        origin: GuardOrigin,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        if !self.origins.contains(&origin) {
            let requested = self.origins.len().checked_add(1).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "specialized guard origins",
                },
            )?;
            check_limit("specialized guard origins", requested, max_guard_origins)?;
            self.origins.insert(origin);
        }
        Ok(())
    }

    pub(crate) fn merge_origins_from(
        &mut self,
        other: &Self,
        max_guard_origins: usize,
    ) -> Result<(), ParametricCoefficientError> {
        debug_assert_eq!(self.polynomial, other.polynomial);
        let additional = other
            .origins
            .iter()
            .filter(|origin| !self.origins.contains(*origin))
            .count();
        let requested = self.origins.len().checked_add(additional).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "specialized guard origins",
            },
        )?;
        check_limit("specialized guard origins", requested, max_guard_origins)?;
        self.origins.extend(other.origins.iter().cloned());
        Ok(())
    }
}

/// The normalized result of a parametric division plus every required
/// pre-cancellation nonzero condition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedParametricCoefficient {
    pub value: ParametricCoefficient,
    pub nonzero: Vec<ParametricNonZeroCondition>,
}

/// Crate-private first phase of guarded division.
///
/// Symbolica has already performed the checked field division, but RustRed's
/// explicit second canonicalization has not run yet.  Elimination uses this
/// narrow seam to census the *actual* normalization input before entering
/// that second native GCD call.  The pending value cannot escape the crate or
/// be mistaken for the public, fully normalized result.
pub(crate) struct PendingGuardedParametricDivision {
    value: ParametricCoefficient,
    nonzero: Vec<ParametricNonZeroCondition>,
}

impl PendingGuardedParametricDivision {
    pub(crate) const fn value_before_final_normalization(&self) -> &ParametricCoefficient {
        &self.value
    }
}

impl ParametricPolynomial {
    pub fn raw(&self) -> &CoefficientPolynomial {
        &self.raw
    }

    /// Authenticated `K(n)` identity retained behind this polynomial's shared
    /// context allocation. Checked certificate comparators use the borrowed
    /// payload to census the complete `Arc<str>` equality work.
    pub(crate) fn authenticated_context_fingerprint(&self) -> &str {
        &self.context
    }

    pub fn to_expression(&self) -> Atom {
        self.raw.to_expression()
    }

    pub fn is_zero(&self) -> bool {
        self.raw.is_zero()
    }

    pub fn is_one(&self) -> bool {
        self.raw.is_one()
    }

    pub fn is_nonzero_constant(&self) -> bool {
        self.raw.is_constant() && !self.raw.is_zero()
    }

    /// Number of sparse monomials retained by the authenticated Symbolica
    /// polynomial.  Proof-bearing layers use this to preflight the memory
    /// cost of duplicating a predicate across complementary case branches.
    pub fn term_count(&self) -> usize {
        self.raw.nterms()
    }

    /// Conservative bytes owned by a deep clone of the sparse Symbolica
    /// payload. The backing-vector capacities and spare GMP limb capacities
    /// are charged, while shared variable/context maps remain `Arc` seams.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        size_of::<Self>().checked_add(polynomial_owned_retained_byte_bound(&self.raw)?)
    }

    /// Fallibly copy the owned sparse payload of an already authenticated
    /// polynomial while retaining the exact Symbolica variable-map allocation.
    ///
    /// Callers must census term, exponent, and integer-bit payload before this
    /// seam.  The two user-sized backing vectors are reserved before any GMP
    /// coefficient is cloned; `variables` and the RustRed context identity are
    /// `Arc`-shared rather than deep-copied.
    pub(crate) fn try_copy_authenticated_sparse_payload(&self) -> Result<Self, &'static str> {
        Ok(Self {
            raw: try_copy_authenticated_sparse_polynomial_payload(&self.raw)?,
            context: self.context.clone(),
        })
    }
}

fn try_copy_authenticated_sparse_polynomial_payload(
    source: &CoefficientPolynomial,
) -> Result<CoefficientPolynomial, &'static str> {
    let mut copy = source.zero();
    copy.coefficients
        .try_reserve_exact(source.coefficients.len())
        .map_err(|_| "authenticated polynomial coefficients")?;
    copy.exponents
        .try_reserve_exact(source.exponents.len())
        .map_err(|_| "authenticated polynomial exponents")?;
    copy.coefficients
        .extend(source.coefficients.iter().cloned());
    copy.exponents.extend_from_slice(&source.exponents);
    Ok(copy)
}

fn polynomial_owned_retained_byte_bound(polynomial: &CoefficientPolynomial) -> Option<usize> {
    let mut bytes = polynomial
        .coefficients
        .capacity()
        .checked_mul(size_of::<Integer>())?
        .checked_add(
            polynomial
                .exponents
                .capacity()
                .checked_mul(size_of::<u16>())?,
        )?;
    for coefficient in &polynomial.coefficients {
        if let Integer::Large(value) = coefficient {
            let capacity_bits = usize::try_from(value.capacity()).ok()?;
            let limb_payload = capacity_bits.checked_add(7)?.checked_div(8)?;
            bytes = bytes.checked_add(limb_payload)?;
        }
    }
    Some(bytes)
}

/// Conservative bytes owned by a deep clone of one exact base-field
/// coefficient. The rational-polynomial header owns both sparse polynomial
/// headers; this adds their backing-vector capacities and spare GMP limbs.
pub(crate) fn coefficient_owned_retained_byte_bound(coefficient: &Coefficient) -> Option<usize> {
    size_of::<Coefficient>()
        .checked_add(polynomial_owned_retained_byte_bound(
            &coefficient.numerator,
        )?)?
        .checked_add(polynomial_owned_retained_byte_bound(
            &coefficient.denominator,
        )?)
}

fn arc_payload_control_and_padding_byte_bound<T>() -> Option<usize> {
    size_of::<usize>()
        .checked_mul(2)?
        .checked_add(align_of::<T>().saturating_sub(1))?
        .checked_add(size_of::<T>())
}

fn check_associate_stats(
    stats: &ParametricPolynomialAssociateStats,
    limits: ParametricPolynomialAssociateLimits,
) -> Result<(), ParametricCoefficientError> {
    for (resource, requested, limit) in [
        (
            "polynomial-associate context fingerprint comparison bytes",
            stats.context_fingerprint_comparison_bytes,
            limits.max_context_fingerprint_comparison_bytes,
        ),
        (
            "polynomial-associate variable-map entry comparisons",
            stats.variable_map_entry_comparisons,
            limits.max_variable_map_entry_comparisons,
        ),
        (
            "polynomial-associate validation terms",
            stats.validation_terms,
            limits.max_validation_terms,
        ),
        (
            "polynomial-associate validation exponent entries",
            stats.validation_exponent_entries,
            limits.max_validation_exponent_entries,
        ),
        (
            "polynomial-associate validation integer bits",
            stats.validation_integer_bits,
            limits.max_validation_integer_bits,
        ),
        (
            "polynomial-associate magnitude copy terms",
            stats.magnitude_copy_terms,
            limits.max_magnitude_copy_terms,
        ),
        (
            "polynomial-associate magnitude copy bytes",
            stats.magnitude_copy_bytes,
            limits.max_magnitude_copy_bytes,
        ),
        (
            "polynomial-associate term-order entries",
            stats.term_order_entries,
            limits.max_term_order_entries,
        ),
        (
            "polynomial-associate index groups",
            stats.index_groups,
            limits.max_index_groups,
        ),
        (
            "polynomial-associate ordering comparison bound",
            stats.ordering_comparison_bound,
            limits.max_ordering_comparison_bound,
        ),
        (
            "polynomial-associate index-coordinate inspection bound",
            stats.index_coordinate_inspection_bound,
            limits.max_index_coordinate_inspection_bound,
        ),
        (
            "polynomial-associate base-sum coordinate inspection bound",
            stats.base_sum_coordinate_inspection_bound,
            limits.max_base_sum_coordinate_inspection_bound,
        ),
        (
            "polynomial-associate base-exponent addition bound",
            stats.base_exponent_addition_bound,
            limits.max_base_exponent_addition_bound,
        ),
        (
            "polynomial-associate anchor cost operations",
            stats.anchor_cost_operations,
            limits.max_anchor_cost_operations,
        ),
        (
            "polynomial-associate cross terms",
            stats.cross_terms,
            limits.max_cross_terms,
        ),
        (
            "polynomial-associate peak cross terms",
            stats.peak_cross_terms,
            limits.max_peak_cross_terms,
        ),
        (
            "polynomial-associate integer preflight pairs",
            stats.integer_preflight_pairs,
            limits.max_integer_preflight_pairs,
        ),
        (
            "polynomial-associate integer multiplication bit-work bound",
            stats.integer_multiplication_bit_work_bound,
            limits.max_integer_multiplication_bit_work_bound,
        ),
        (
            "polynomial-associate integer accumulation bit-work bound",
            stats.integer_accumulation_bit_work_bound,
            limits.max_integer_accumulation_bit_work_bound,
        ),
        (
            "polynomial-associate integer multiply limb-operation bound",
            stats.integer_multiply_limb_operation_bound,
            limits.max_integer_multiply_limb_operation_bound,
        ),
        (
            "polynomial-associate integer accumulation limb-operation bound",
            stats.integer_accumulate_limb_operation_bound,
            limits.max_integer_accumulate_limb_operation_bound,
        ),
        (
            "polynomial-associate scratch limb-write bound",
            stats.scratch_limb_write_bound,
            limits.max_scratch_limb_write_bound,
        ),
        (
            "polynomial-associate product scratch limbs",
            stats.product_scratch_limbs,
            limits.max_product_scratch_limbs,
        ),
        (
            "polynomial-associate accumulator scratch limbs",
            stats.accumulator_scratch_limbs,
            limits.max_accumulator_scratch_limbs,
        ),
        (
            "polynomial-associate visible temporary byte envelope",
            stats.visible_temporary_byte_envelope,
            limits.max_visible_temporary_byte_envelope,
        ),
    ] {
        check_limit(resource, requested, limit)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AssociateIndexGroup {
    begin: usize,
    end: usize,
}

impl AssociateIndexGroup {
    fn len(self) -> usize {
        self.end - self.begin
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AssociateMagnitudeSpan {
    offset: usize,
    byte_len: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AssociateCrossRef {
    p_term: usize,
    q_term: usize,
    rhs: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AssociateObservedWork {
    ordering_comparisons: usize,
    integer_multiply_limb_operations: usize,
    integer_accumulate_limb_operations: usize,
    scratch_limb_writes: usize,
}

fn associate_heap_comparison_bound(terms: usize) -> Result<usize, ParametricCoefficientError> {
    if terms < 2 {
        return Ok(0);
    }
    checked_parametric_mul(
        "polynomial-associate ordering comparison bound",
        checked_parametric_mul("polynomial-associate ordering comparison bound", 4, terms)?,
        checked_parametric_add(
            "polynomial-associate ordering comparison bound",
            parametric_ceil_log2(terms),
            1,
        )?,
    )
}

fn associate_heap_sort_by<T>(
    values: &mut [T],
    mut compare: impl FnMut(&T, &T) -> Ordering,
    comparison_bound: usize,
) -> Result<usize, ParametricCoefficientError> {
    fn sift_down<T>(
        values: &mut [T],
        mut root: usize,
        end: usize,
        compare: &mut impl FnMut(&T, &T) -> Ordering,
        comparisons: &mut usize,
        comparison_bound: usize,
    ) -> Result<(), ParametricCoefficientError> {
        loop {
            let Some(mut child) = root.checked_mul(2).and_then(|value| value.checked_add(1)) else {
                return Err(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial-associate heap child",
                });
            };
            if child >= end {
                break;
            }
            if child + 1 < end {
                *comparisons = checked_parametric_add(
                    "polynomial-associate observed ordering comparisons",
                    *comparisons,
                    1,
                )?;
                if *comparisons > comparison_bound {
                    return Err(ParametricCoefficientError::ResourceLimit {
                        resource: "polynomial-associate observed ordering comparisons",
                        requested: *comparisons,
                        limit: comparison_bound,
                    });
                }
                if compare(&values[child], &values[child + 1]) == Ordering::Less {
                    child += 1;
                }
            }
            *comparisons = checked_parametric_add(
                "polynomial-associate observed ordering comparisons",
                *comparisons,
                1,
            )?;
            if *comparisons > comparison_bound {
                return Err(ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate observed ordering comparisons",
                    requested: *comparisons,
                    limit: comparison_bound,
                });
            }
            if compare(&values[root], &values[child]) != Ordering::Less {
                break;
            }
            values.swap(root, child);
            root = child;
        }
        Ok(())
    }

    let mut comparisons = 0usize;
    let len = values.len();
    for root in (0..len / 2).rev() {
        sift_down(
            values,
            root,
            len,
            &mut compare,
            &mut comparisons,
            comparison_bound,
        )?;
    }
    for end in (1..len).rev() {
        values.swap(0, end);
        sift_down(
            values,
            0,
            end,
            &mut compare,
            &mut comparisons,
            comparison_bound,
        )?;
    }
    Ok(comparisons)
}

fn associate_term_index_cmp(
    polynomial: &CoefficientPolynomial,
    left: usize,
    right: usize,
    first_index: usize,
) -> Ordering {
    polynomial.exponents(left)[first_index..]
        .cmp(&polynomial.exponents(right)[first_index..])
        .then_with(|| left.cmp(&right))
}

fn associate_same_index_key(
    left: &CoefficientPolynomial,
    left_term: usize,
    right: &CoefficientPolynomial,
    right_term: usize,
    first_index: usize,
) -> bool {
    left.exponents(left_term)[first_index..] == right.exponents(right_term)[first_index..]
}

fn associate_count_groups(
    polynomial: &CoefficientPolynomial,
    order: &[usize],
    first_index: usize,
) -> usize {
    if order.is_empty() {
        return 0;
    }
    1 + order
        .windows(2)
        .filter(|pair| {
            !associate_same_index_key(polynomial, pair[0], polynomial, pair[1], first_index)
        })
        .count()
}

fn associate_build_groups(
    polynomial: &CoefficientPolynomial,
    order: &[usize],
    first_index: usize,
    expected_groups: usize,
) -> Result<Vec<AssociateIndexGroup>, ParametricCoefficientError> {
    let mut groups = Vec::new();
    groups.try_reserve_exact(expected_groups).map_err(|_| {
        ParametricCoefficientError::Symbolica(
            "allocation failed for polynomial-associate index groups".to_owned(),
        )
    })?;
    if order.is_empty() {
        return Ok(groups);
    }
    let mut begin = 0usize;
    for end in 1..order.len() {
        if !associate_same_index_key(
            polynomial,
            order[end - 1],
            polynomial,
            order[end],
            first_index,
        ) {
            if groups.len() >= expected_groups {
                return Err(ParametricCoefficientError::Symbolica(
                    "internal polynomial-associate group capacity exhausted".to_owned(),
                ));
            }
            groups.push(AssociateIndexGroup { begin, end });
            begin = end;
        }
    }
    if groups.len() >= expected_groups {
        return Err(ParametricCoefficientError::Symbolica(
            "internal polynomial-associate group capacity exhausted".to_owned(),
        ));
    }
    groups.push(AssociateIndexGroup {
        begin,
        end: order.len(),
    });
    if groups.len() != expected_groups {
        return Err(ParametricCoefficientError::Symbolica(
            "internal polynomial-associate group census mismatch".to_owned(),
        ));
    }
    Ok(groups)
}

fn associate_cross_term_count(
    p_group: AssociateIndexGroup,
    q_group: AssociateIndexGroup,
    p_anchor: AssociateIndexGroup,
    q_anchor: AssociateIndexGroup,
) -> Result<usize, ParametricCoefficientError> {
    checked_parametric_add(
        "polynomial-associate cross terms",
        checked_parametric_mul(
            "polynomial-associate cross terms",
            p_group.len(),
            q_anchor.len(),
        )?,
        checked_parametric_mul(
            "polynomial-associate cross terms",
            p_anchor.len(),
            q_group.len(),
        )?,
    )
}

fn associate_visit_cross_pairs(
    p_order: &[usize],
    q_order: &[usize],
    p_group: AssociateIndexGroup,
    q_group: AssociateIndexGroup,
    p_anchor: AssociateIndexGroup,
    q_anchor: AssociateIndexGroup,
    mut visitor: impl FnMut(usize, usize, bool) -> Result<(), ParametricCoefficientError>,
) -> Result<(), ParametricCoefficientError> {
    for &p_term in &p_order[p_group.begin..p_group.end] {
        for &q_term in &q_order[q_anchor.begin..q_anchor.end] {
            visitor(p_term, q_term, false)?;
        }
    }
    for &p_term in &p_order[p_anchor.begin..p_anchor.end] {
        for &q_term in &q_order[q_group.begin..q_group.end] {
            visitor(p_term, q_term, true)?;
        }
    }
    Ok(())
}

fn associate_cross_ref_cmp(
    p: &CoefficientPolynomial,
    q: &CoefficientPolynomial,
    left: &AssociateCrossRef,
    right: &AssociateCrossRef,
    base_variables: usize,
) -> Ordering {
    let p_left = p.exponents(left.p_term);
    let q_left = q.exponents(left.q_term);
    let p_right = p.exponents(right.p_term);
    let q_right = q.exponents(right.q_term);
    for variable in 0..base_variables {
        let left_sum = u32::from(p_left[variable]) + u32::from(q_left[variable]);
        let right_sum = u32::from(p_right[variable]) + u32::from(q_right[variable]);
        match left_sum.cmp(&right_sum) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    left.rhs
        .cmp(&right.rhs)
        .then_with(|| left.p_term.cmp(&right.p_term))
        .then_with(|| left.q_term.cmp(&right.q_term))
}

fn associate_same_cross_key(
    p: &CoefficientPolynomial,
    q: &CoefficientPolynomial,
    left: &AssociateCrossRef,
    right: &AssociateCrossRef,
    base_variables: usize,
) -> bool {
    let p_left = p.exponents(left.p_term);
    let q_left = q.exponents(left.q_term);
    let p_right = p.exponents(right.p_term);
    let q_right = q.exponents(right.q_term);
    (0..base_variables).all(|variable| {
        u32::from(p_left[variable]) + u32::from(q_left[variable])
            == u32::from(p_right[variable]) + u32::from(q_right[variable])
    })
}

fn associate_integer_bit_count(value: &Integer) -> Result<usize, ParametricCoefficientError> {
    usize::try_from(integer_magnitude_bits(value)).map_err(|_| {
        ParametricCoefficientError::ResourceCountOverflow {
            resource: "polynomial-associate integer bits",
        }
    })
}

fn associate_bits_to_units(
    resource: &'static str,
    bits: usize,
    unit_bits: usize,
) -> Result<usize, ParametricCoefficientError> {
    if bits == 0 {
        return Ok(0);
    }
    bits.checked_add(unit_bits - 1)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
        .map(|value| value / unit_bits)
}

fn associate_capacity_byte_envelope(
    entries: usize,
    entry_size: usize,
) -> Result<usize, ParametricCoefficientError> {
    checked_parametric_mul(
        "polynomial-associate visible temporary byte envelope",
        checked_parametric_mul(
            "polynomial-associate visible temporary byte envelope",
            entries,
            2,
        )?,
        entry_size,
    )
}

fn associate_check_capacity(
    capacity: usize,
    requested: usize,
    resource: &'static str,
) -> Result<(), ParametricCoefficientError> {
    let limit = requested
        .checked_mul(2)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })?;
    if capacity > limit {
        Err(ParametricCoefficientError::ResourceLimit {
            resource,
            requested: capacity,
            limit,
        })
    } else {
        Ok(())
    }
}

fn associate_sum_counts(
    resource: &'static str,
    values: impl IntoIterator<Item = usize>,
) -> Result<usize, ParametricCoefficientError> {
    values.into_iter().try_fold(0usize, |sum, value| {
        checked_parametric_add(resource, sum, value)
    })
}

fn associate_append_magnitude(
    value: &Integer,
    arena: &mut Vec<u8>,
) -> Result<AssociateMagnitudeSpan, ParametricCoefficientError> {
    let bits = associate_integer_bit_count(value)?;
    let byte_len = associate_bits_to_units("polynomial-associate magnitude copy bytes", bits, 8)?;
    let offset = arena.len();
    let available_capacity = arena.capacity().checked_sub(offset).ok_or(
        ParametricCoefficientError::ResourceCountOverflow {
            resource: "polynomial-associate magnitude copy bytes",
        },
    )?;
    if byte_len > available_capacity {
        return Err(ParametricCoefficientError::ResourceLimit {
            resource: "polynomial-associate magnitude copy bytes",
            requested: byte_len,
            limit: available_capacity,
        });
    }
    match value {
        Integer::Single(value) => {
            let bytes = value.unsigned_abs().to_le_bytes();
            arena.extend_from_slice(&bytes[..byte_len]);
        }
        Integer::Double(value) => {
            let bytes = value.unsigned_abs().to_le_bytes();
            arena.extend_from_slice(&bytes[..byte_len]);
        }
        Integer::Large(value) => {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                write_lsf_bytes(value, arena);
            }))
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "Symbolica panicked while exporting polynomial-associate magnitude bytes"
                        .to_owned(),
                )
            })?;
        }
    }
    let observed = arena.len().checked_sub(offset).ok_or(
        ParametricCoefficientError::ResourceCountOverflow {
            resource: "polynomial-associate magnitude copy bytes",
        },
    )?;
    if observed != byte_len {
        return Err(ParametricCoefficientError::Symbolica(
            "internal polynomial-associate magnitude length mismatch".to_owned(),
        ));
    }
    Ok(AssociateMagnitudeSpan { offset, byte_len })
}

fn associate_magnitude_limb(
    arena: &[u8],
    span: AssociateMagnitudeSpan,
    limb: usize,
) -> Result<u64, ParametricCoefficientError> {
    let relative =
        limb.checked_mul(8)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "polynomial-associate magnitude limb offset",
            })?;
    if relative >= span.byte_len {
        return Ok(0);
    }
    let start = span.offset.checked_add(relative).ok_or(
        ParametricCoefficientError::ResourceCountOverflow {
            resource: "polynomial-associate magnitude limb offset",
        },
    )?;
    let available = (span.byte_len - relative).min(8);
    let end =
        start
            .checked_add(available)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "polynomial-associate magnitude limb offset",
            })?;
    let bytes = arena.get(start..end).ok_or_else(|| {
        ParametricCoefficientError::Symbolica(
            "internal polynomial-associate magnitude span mismatch".to_owned(),
        )
    })?;
    let mut packed = [0u8; 8];
    packed[..available].copy_from_slice(bytes);
    Ok(u64::from_le_bytes(packed))
}

fn associate_multiply_magnitudes(
    arena: &[u8],
    left: AssociateMagnitudeSpan,
    right: AssociateMagnitudeSpan,
    product: &mut [u64],
    observed: &mut AssociateObservedWork,
) -> Result<usize, ParametricCoefficientError> {
    let left_limbs = associate_bits_to_units(
        "polynomial-associate product scratch limbs",
        left.byte_len
            .checked_mul(8)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "polynomial-associate product scratch limbs",
            })?,
        64,
    )?;
    let right_limbs = associate_bits_to_units(
        "polynomial-associate product scratch limbs",
        right
            .byte_len
            .checked_mul(8)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "polynomial-associate product scratch limbs",
            })?,
        64,
    )?;
    let needed = checked_parametric_add(
        "polynomial-associate product scratch limbs",
        left_limbs,
        right_limbs,
    )?;
    if needed > product.len() {
        return Err(ParametricCoefficientError::ResourceLimit {
            resource: "polynomial-associate product scratch limbs",
            requested: needed,
            limit: product.len(),
        });
    }
    product[..needed].fill(0);
    observed.scratch_limb_writes = checked_parametric_add(
        "polynomial-associate observed scratch limb writes",
        observed.scratch_limb_writes,
        needed,
    )?;
    for left_limb in 0..left_limbs {
        let a = u128::from(associate_magnitude_limb(arena, left, left_limb)?);
        let mut carry = 0u128;
        for right_limb in 0..right_limbs {
            let index = left_limb + right_limb;
            let b = u128::from(associate_magnitude_limb(arena, right, right_limb)?);
            let wide = a * b + u128::from(product[index]) + carry;
            product[index] = wide as u64;
            carry = wide >> 64;
            observed.integer_multiply_limb_operations = checked_parametric_add(
                "polynomial-associate observed integer multiply limb operations",
                observed.integer_multiply_limb_operations,
                1,
            )?;
            observed.scratch_limb_writes = checked_parametric_add(
                "polynomial-associate observed scratch limb writes",
                observed.scratch_limb_writes,
                1,
            )?;
        }
        product[left_limb + right_limbs] = carry as u64;
        observed.scratch_limb_writes = checked_parametric_add(
            "polynomial-associate observed scratch limb writes",
            observed.scratch_limb_writes,
            1,
        )?;
    }
    let mut len = needed;
    while len > 0 && product[len - 1] == 0 {
        len -= 1;
    }
    if len == 0 {
        return Err(ParametricCoefficientError::Symbolica(
            "internal polynomial-associate nonzero product vanished".to_owned(),
        ));
    }
    Ok(len)
}

fn associate_magnitude_cmp(
    left: &[u64],
    left_len: usize,
    right: &[u64],
    right_len: usize,
) -> Ordering {
    match left_len.cmp(&right_len) {
        Ordering::Equal => {}
        ordering => return ordering,
    }
    for index in (0..left_len).rev() {
        match left[index].cmp(&right[index]) {
            Ordering::Equal => {}
            ordering => return ordering,
        }
    }
    Ordering::Equal
}

fn associate_accumulate_product(
    accumulator: &mut [u64],
    accumulator_len: &mut usize,
    accumulator_negative: &mut bool,
    product: &[u64],
    product_len: usize,
    product_negative: bool,
    observed: &mut AssociateObservedWork,
) -> Result<(), ParametricCoefficientError> {
    if *accumulator_len == 0 {
        accumulator[..product_len].copy_from_slice(&product[..product_len]);
        *accumulator_len = product_len;
        *accumulator_negative = product_negative;
        observed.scratch_limb_writes = checked_parametric_add(
            "polynomial-associate observed scratch limb writes",
            observed.scratch_limb_writes,
            product_len,
        )?;
        return Ok(());
    }

    if *accumulator_negative == product_negative {
        let previous_len = *accumulator_len;
        let common_len = previous_len.max(product_len);
        let mut carry = 0u128;
        for index in 0..common_len {
            let left = if index < previous_len {
                accumulator[index]
            } else {
                0
            };
            let right = if index < product_len {
                product[index]
            } else {
                0
            };
            let wide = u128::from(left) + u128::from(right) + carry;
            accumulator[index] = wide as u64;
            carry = wide >> 64;
            observed.integer_accumulate_limb_operations = checked_parametric_add(
                "polynomial-associate observed integer accumulation limb operations",
                observed.integer_accumulate_limb_operations,
                1,
            )?;
            observed.scratch_limb_writes = checked_parametric_add(
                "polynomial-associate observed scratch limb writes",
                observed.scratch_limb_writes,
                1,
            )?;
        }
        *accumulator_len = common_len;
        if carry != 0 {
            let accumulator_capacity = accumulator.len();
            let requested = checked_parametric_add(
                "polynomial-associate accumulator scratch limbs",
                *accumulator_len,
                1,
            )?;
            let slot = accumulator.get_mut(*accumulator_len).ok_or(
                ParametricCoefficientError::ResourceLimit {
                    resource: "polynomial-associate accumulator scratch limbs",
                    requested,
                    limit: accumulator_capacity,
                },
            )?;
            *slot = carry as u64;
            *accumulator_len += 1;
            observed.scratch_limb_writes = checked_parametric_add(
                "polynomial-associate observed scratch limb writes",
                observed.scratch_limb_writes,
                1,
            )?;
        }
        return Ok(());
    }

    let ordering = associate_magnitude_cmp(accumulator, *accumulator_len, product, product_len);
    observed.integer_accumulate_limb_operations = checked_parametric_add(
        "polynomial-associate observed integer accumulation limb operations",
        observed.integer_accumulate_limb_operations,
        (*accumulator_len).max(product_len),
    )?;
    if ordering == Ordering::Equal {
        *accumulator_len = 0;
        *accumulator_negative = false;
        return Ok(());
    }

    let result_negative = if ordering == Ordering::Greater {
        *accumulator_negative
    } else {
        product_negative
    };
    let result_len = (*accumulator_len).max(product_len);
    let mut borrow = false;
    for index in 0..result_len {
        let accumulator_value = if index < *accumulator_len {
            accumulator[index]
        } else {
            0
        };
        let product_value = if index < product_len {
            product[index]
        } else {
            0
        };
        let (larger, smaller) = if ordering == Ordering::Greater {
            (accumulator_value, product_value)
        } else {
            (product_value, accumulator_value)
        };
        let (difference, first_borrow) = larger.overflowing_sub(smaller);
        let borrow_limb = if borrow { 1 } else { 0 };
        let (difference, second_borrow) = difference.overflowing_sub(borrow_limb);
        accumulator[index] = difference;
        borrow = first_borrow || second_borrow;
        observed.integer_accumulate_limb_operations = checked_parametric_add(
            "polynomial-associate observed integer accumulation limb operations",
            observed.integer_accumulate_limb_operations,
            1,
        )?;
        observed.scratch_limb_writes = checked_parametric_add(
            "polynomial-associate observed scratch limb writes",
            observed.scratch_limb_writes,
            1,
        )?;
    }
    if borrow {
        return Err(ParametricCoefficientError::Symbolica(
            "internal polynomial-associate magnitude subtraction underflow".to_owned(),
        ));
    }
    *accumulator_len = result_len;
    while *accumulator_len > 0 && accumulator[*accumulator_len - 1] == 0 {
        *accumulator_len -= 1;
    }
    *accumulator_negative = if *accumulator_len == 0 {
        false
    } else {
        result_negative
    };
    Ok(())
}

/// Explicit upper bounds around Symbolica operations whose output can expand
/// under an affine index translation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParametricArithmeticLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_source_terms: usize,
    pub max_output_terms: usize,
    pub max_specialization_power_operations: usize,
    /// Maximum conservative magnitude bit length of an integer coefficient
    /// produced while specializing or affinely translating index variables.
    pub max_specialization_integer_bits: usize,
    pub max_guard_origins: usize,
}

impl Default for ParametricArithmeticLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_source_terms: 1_000_000,
            max_output_terms: 4_000_000,
            max_specialization_power_operations: 16_000_000,
            max_specialization_integer_bits: 16_000_000,
            max_guard_origins: 65_536,
        }
    }
}

/// Allocation-free prospective census for one polynomial translation
/// `n -> n + shift`.
///
/// `output_*_bound` describes the expanded polynomial before any rational
/// normalization. `retained_output_byte_bound` includes the authenticated
/// polynomial wrapper, its dense exponent payload, and a limb-rounded GMP
/// payload for every prospective coefficient; the variable map and context
/// fingerprint remain shared `Arc` seams.  It is a successful-output bound,
/// not a peak bound for Symbolica's native replacement intermediates.  An
/// enclosing group/database plan must separately admit aggregate integer-bit
/// work and visible/native temporary-memory envelopes before execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricPolynomialTranslationPreflight {
    source_terms: usize,
    source_exponent_entries: usize,
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    power_operation_bound: usize,
    largest_output_integer_bit_bound: usize,
    largest_output_integer_capacity_byte_bound: usize,
    output_coefficient_capacity_bound: usize,
    output_exponent_capacity_bound: usize,
    integer_bit_work_bound: usize,
    retained_output_term_bound: usize,
    retained_output_byte_bound: usize,
}

impl ParametricPolynomialTranslationPreflight {
    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn source_exponent_entries(self) -> usize {
        self.source_exponent_entries
    }

    pub(crate) const fn output_term_bound(self) -> usize {
        self.output_term_bound
    }

    pub(crate) const fn output_exponent_entry_bound(self) -> usize {
        self.output_exponent_entry_bound
    }

    pub(crate) const fn power_operation_bound(self) -> usize {
        self.power_operation_bound
    }

    pub(crate) const fn largest_output_integer_bit_bound(self) -> usize {
        self.largest_output_integer_bit_bound
    }

    pub(crate) const fn integer_bit_work_bound(self) -> usize {
        self.integer_bit_work_bound
    }

    pub(crate) const fn retained_output_term_bound(self) -> usize {
        self.retained_output_term_bound
    }

    pub(crate) const fn retained_output_byte_bound(self) -> usize {
        self.retained_output_byte_bound
    }
}

/// Allocation-free prospective census for projection from `K(n)` to `K` at
/// one complete integer assignment.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricPolynomialSpecializationPreflight {
    source_terms: usize,
    source_exponent_entries: usize,
    output_term_bound: usize,
    output_exponent_entry_bound: usize,
    power_operation_bound: usize,
    largest_output_integer_bit_bound: usize,
    largest_output_integer_capacity_byte_bound: usize,
    output_coefficient_capacity_bound: usize,
    output_exponent_capacity_bound: usize,
    integer_bit_work_bound: usize,
    retained_output_term_bound: usize,
    retained_output_byte_bound: usize,
}

impl ParametricPolynomialSpecializationPreflight {
    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn source_exponent_entries(self) -> usize {
        self.source_exponent_entries
    }

    pub(crate) const fn output_term_bound(self) -> usize {
        self.output_term_bound
    }

    pub(crate) const fn output_exponent_entry_bound(self) -> usize {
        self.output_exponent_entry_bound
    }

    pub(crate) const fn power_operation_bound(self) -> usize {
        self.power_operation_bound
    }

    pub(crate) const fn largest_output_integer_bit_bound(self) -> usize {
        self.largest_output_integer_bit_bound
    }

    pub(crate) const fn integer_bit_work_bound(self) -> usize {
        self.integer_bit_work_bound
    }

    pub(crate) const fn retained_output_term_bound(self) -> usize {
        self.retained_output_term_bound
    }

    pub(crate) const fn retained_output_byte_bound(self) -> usize {
        self.retained_output_byte_bound
    }
}

/// Complete prospective census for translating one rational coefficient.
/// The normalized bounds cover the successful post-GCD `K(n)` value, not the
/// transient native GCD workspace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricCoefficientTranslationPreflight {
    numerator: ParametricPolynomialTranslationPreflight,
    denominator: ParametricPolynomialTranslationPreflight,
    source_terms: usize,
    output_term_bound: usize,
    power_operation_bound: usize,
    integer_bit_work_bound: usize,
    normalization_input_term_pair_bound: usize,
    normalized_coefficient_term_bound: usize,
    normalized_coefficient_byte_bound: usize,
}

impl ParametricCoefficientTranslationPreflight {
    pub(crate) const fn numerator(self) -> ParametricPolynomialTranslationPreflight {
        self.numerator
    }

    pub(crate) const fn denominator(self) -> ParametricPolynomialTranslationPreflight {
        self.denominator
    }

    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn output_term_bound(self) -> usize {
        self.output_term_bound
    }

    pub(crate) const fn power_operation_bound(self) -> usize {
        self.power_operation_bound
    }

    pub(crate) const fn integer_bit_work_bound(self) -> usize {
        self.integer_bit_work_bound
    }

    pub(crate) const fn normalization_input_term_pair_bound(self) -> usize {
        self.normalization_input_term_pair_bound
    }

    pub(crate) const fn normalized_coefficient_term_bound(self) -> usize {
        self.normalized_coefficient_term_bound
    }

    pub(crate) const fn normalized_coefficient_byte_bound(self) -> usize {
        self.normalized_coefficient_byte_bound
    }
}

/// Complete prospective census for concretely specializing one rational
/// coefficient. The mapped denominator is budgeted independently because it
/// is retained as a nonzero guard before fraction normalization can cancel it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ParametricCoefficientSpecializationPreflight {
    numerator: ParametricPolynomialSpecializationPreflight,
    denominator: ParametricPolynomialSpecializationPreflight,
    source_terms: usize,
    output_term_bound: usize,
    power_operation_bound: usize,
    integer_bit_work_bound: usize,
    normalization_input_term_pair_bound: usize,
    normalized_coefficient_term_bound: usize,
    normalized_coefficient_byte_bound: usize,
    denominator_guard_term_bound: usize,
    denominator_guard_byte_bound: usize,
}

impl ParametricCoefficientSpecializationPreflight {
    pub(crate) const fn numerator(self) -> ParametricPolynomialSpecializationPreflight {
        self.numerator
    }

    pub(crate) const fn denominator(self) -> ParametricPolynomialSpecializationPreflight {
        self.denominator
    }

    pub(crate) const fn source_terms(self) -> usize {
        self.source_terms
    }

    pub(crate) const fn output_term_bound(self) -> usize {
        self.output_term_bound
    }

    pub(crate) const fn power_operation_bound(self) -> usize {
        self.power_operation_bound
    }

    pub(crate) const fn integer_bit_work_bound(self) -> usize {
        self.integer_bit_work_bound
    }

    pub(crate) const fn normalization_input_term_pair_bound(self) -> usize {
        self.normalization_input_term_pair_bound
    }

    pub(crate) const fn normalized_coefficient_term_bound(self) -> usize {
        self.normalized_coefficient_term_bound
    }

    pub(crate) const fn normalized_coefficient_byte_bound(self) -> usize {
        self.normalized_coefficient_byte_bound
    }

    pub(crate) const fn denominator_guard_term_bound(self) -> usize {
        self.denominator_guard_term_bound
    }

    pub(crate) const fn denominator_guard_byte_bound(self) -> usize {
        self.denominator_guard_byte_bound
    }
}

/// Stable schema for bounded simultaneous composition through a certified
/// residual unit-affine index map.
pub const RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA: &str =
    "rustred-residual-unit-affine-composition-v1";

/// Internal algebra-only core shared by provenance-bearing V1 adapters and
/// authority-neutral compact V2 plans.  This schema authenticates no map
/// derivation or certificate provenance.
const RESIDUAL_AFFINE_COMPOSITION_CORE_V1_SCHEMA: &str =
    "rustred-residual-affine-composition-core-v1";

/// Stable schema for authority-neutral composition through an authenticated
/// compact affine geometry.
///
/// Unlike the V1 integer-system adapter, this schema makes no claim about how
/// the map was derived.  A proof-bearing owner must retain its own authority
/// and call exact replay with a fresh borrowed geometry view.
pub const RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA: &str =
    "rustred-residual-affine-compact-composition-v2";

#[cfg(test)]
thread_local! {
    static RESIDUAL_AFFINE_COMPACT_BOUNDARY_PANIC_FOR_TEST: std::cell::Cell<bool> =
        const { std::cell::Cell::new(false) };
    static RESIDUAL_AFFINE_COMPACT_POLYNOMIAL_PREFLIGHT_CALLS_FOR_TEST:
        std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
fn inject_residual_affine_compact_boundary_panic_for_test() {
    RESIDUAL_AFFINE_COMPACT_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| panic_next.set(true));
}

#[cfg(test)]
fn maybe_inject_residual_affine_compact_boundary_panic_for_test() {
    RESIDUAL_AFFINE_COMPACT_BOUNDARY_PANIC_FOR_TEST.with(|panic_next| {
        if panic_next.replace(false) {
            panic!("injected compact affine composition boundary panic");
        }
    });
}

#[cfg(test)]
fn reset_residual_affine_compact_preflight_calls_for_test() {
    RESIDUAL_AFFINE_COMPACT_POLYNOMIAL_PREFLIGHT_CALLS_FOR_TEST.with(|calls| calls.set(0));
}

#[cfg(test)]
fn residual_affine_compact_preflight_calls_for_test() -> usize {
    RESIDUAL_AFFINE_COMPACT_POLYNOMIAL_PREFLIGHT_CALLS_FOR_TEST.with(std::cell::Cell::get)
}

#[cfg(test)]
fn note_residual_affine_compact_preflight_call_for_test() {
    RESIDUAL_AFFINE_COMPACT_POLYNOMIAL_PREFLIGHT_CALLS_FOR_TEST.with(|calls| {
        calls.set(
            calls
                .get()
                .checked_add(1)
                .expect("test-only compact affine preflight counter overflow"),
        );
    });
}

/// Allocation bounds for the immutable full-point substitution plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualUnitAffineCompositionPlanLimits {
    pub max_variables: usize,
    pub max_full_images: usize,
    /// Source geometry entries inspected before compact retention. Ambient-
    /// square integer-system maps charge the complete `b` and `A` payload.
    pub max_geometry_entries_inspected: usize,
    /// Integer entries retained by the compact `b` and `B` geometry, where
    /// `B[row, free_ordinal] = A[row, free_positions[free_ordinal]]`.
    pub max_geometry_entries_retained: usize,
    /// Retained sorted pivot/free support positions.
    /// The source-neutral core also charges its row-major Boolean linear
    /// support (`ambient_arity * free_count`) here.
    pub max_support_entries_retained: usize,
    pub max_total_image_terms: usize,
    pub max_total_image_exponent_entries: usize,
    pub max_image_integer_bits: usize,
    /// Exact sum of magnitude bits in every retained full-point image
    /// coefficient, including base-variable and free-row unit images.
    pub max_total_image_integer_bits: usize,
}

impl Default for ResidualUnitAffineCompositionPlanLimits {
    fn default() -> Self {
        Self {
            max_variables: 8_192,
            max_full_images: 8_192,
            max_geometry_entries_inspected: 67_117_056,
            max_geometry_entries_retained: 67_117_056,
            max_support_entries_retained: 67_117_056,
            max_total_image_terms: 16_777_216,
            max_total_image_exponent_entries: 268_435_456,
            max_image_integer_bits: 1_000_000,
            max_total_image_integer_bits: 16_000_000_000,
        }
    }
}

/// Allocation and comparison bounds for an authority-neutral compact affine
/// composition plan.
///
/// `composition` retains the existing exact term, exponent, and image-GMP
/// limits.  The additional limits close the previously implicit V2 seams:
/// context comparison, complete compact-geometry integer inspection, and the
/// allocation-independent logical bytes retained/reached by compilation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineCompactCompositionPlanLimits {
    pub composition: ResidualUnitAffineCompositionPlanLimits,
    pub max_context_fingerprint_bytes: usize,
    pub max_geometry_integer_bit_work: usize,
    /// Exact successful-path comparison census for componentwise replay of
    /// the canonical Symbolica images. Replay is single-pass in this budget.
    pub max_geometry_replay_comparison_work: usize,
    /// Exact successful-path magnitude-bit census for geometry inspection and
    /// the one coefficient equality performed for each retained image term.
    pub max_geometry_replay_integer_bit_work: usize,
    /// Allocation-independent logical bytes in the two replay lookup tables.
    pub max_geometry_replay_scratch_logical_bytes: usize,
    pub max_retained_owned_logical_bytes: usize,
    pub max_compilation_owned_logical_peak_upper_bound: usize,
}

impl Default for ResidualAffineCompactCompositionPlanLimits {
    fn default() -> Self {
        Self {
            composition: ResidualUnitAffineCompositionPlanLimits::default(),
            max_context_fingerprint_bytes: 1024 * 1024,
            max_geometry_integer_bit_work: 16_000_000_000,
            max_geometry_replay_comparison_work: 1_000_000_000,
            max_geometry_replay_integer_bit_work: 32_000_000_000,
            max_geometry_replay_scratch_logical_bytes: 1024 * 1024 * 1024,
            max_retained_owned_logical_bytes: usize::MAX,
            max_compilation_owned_logical_peak_upper_bound: usize::MAX,
        }
    }
}

macro_rules! residual_affine_stats_getters {
    ($($field:ident),* $(,)?) => {$ (
        pub const fn $field(self) -> usize { self.$field }
    )* };
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineCompositionPlanStats {
    variables: usize,
    full_images: usize,
    geometry_entries_inspected: usize,
    geometry_entries_retained: usize,
    support_entries_retained: usize,
    total_image_terms: usize,
    total_image_exponent_entries: usize,
    largest_image_integer_bits: usize,
    total_image_integer_bits: usize,
}

impl ResidualAffineCompositionPlanStats {
    residual_affine_stats_getters!(
        variables,
        full_images,
        geometry_entries_inspected,
        geometry_entries_retained,
        support_entries_retained,
        total_image_terms,
        total_image_exponent_entries,
        largest_image_integer_bits,
        total_image_integer_bits,
    );
}

/// Borrowed, authority-neutral compact affine map.
///
/// `compact_linear_coefficients` is row-major with shape
/// `ambient_arity * free_positions.len()`.  The view borrows exact integers;
/// construction performs no allocation and compilation authenticates every
/// shape and affine-idempotence invariant before cloning any GMP payload.
#[derive(Clone, Copy)]
pub(crate) struct ResidualAffineCompactMapView<'geometry> {
    context_fingerprint: &'geometry str,
    ambient_arity: usize,
    constants: &'geometry [Integer],
    free_positions: &'geometry [usize],
    compact_linear_coefficients: &'geometry [Integer],
}

impl fmt::Debug for ResidualAffineCompactMapView<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineCompactMapView")
            .field("context_fingerprint", &"<redacted>")
            .field("context_fingerprint_bytes", &self.context_fingerprint.len())
            .field("ambient_arity", &self.ambient_arity)
            .field("constant_entries", &self.constants.len())
            .field("free_positions", &self.free_positions)
            .field(
                "compact_linear_entries",
                &self.compact_linear_coefficients.len(),
            )
            .finish()
    }
}

impl<'geometry> ResidualAffineCompactMapView<'geometry> {
    pub(crate) const fn new(
        context_fingerprint: &'geometry str,
        ambient_arity: usize,
        constants: &'geometry [Integer],
        free_positions: &'geometry [usize],
        compact_linear_coefficients: &'geometry [Integer],
    ) -> Self {
        Self {
            context_fingerprint,
            ambient_arity,
            constants,
            free_positions,
            compact_linear_coefficients,
        }
    }

    pub(crate) const fn context_fingerprint(self) -> &'geometry str {
        self.context_fingerprint
    }

    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }

    pub(crate) const fn constants(self) -> &'geometry [Integer] {
        self.constants
    }

    pub(crate) const fn free_positions(self) -> &'geometry [usize] {
        self.free_positions
    }

    pub(crate) const fn compact_linear_coefficients(self) -> &'geometry [Integer] {
        self.compact_linear_coefficients
    }

    fn linear_coefficient(self, row: usize, free_ordinal: usize) -> Option<&'geometry Integer> {
        let offset = row
            .checked_mul(self.free_positions.len())?
            .checked_add(free_ordinal)?;
        self.compact_linear_coefficients.get(offset)
    }
}

/// Exact prospective census computed from a compact geometry before its first
/// owned vector or GMP integer is allocated.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ResidualAffineCompactCompositionPlanStats {
    composition: ResidualAffineCompositionPlanStats,
    context_fingerprint_bytes: usize,
    geometry_integer_entries_inspected: usize,
    geometry_integer_bit_work: usize,
    geometry_replay_comparison_work: usize,
    geometry_replay_integer_bit_work: usize,
    geometry_replay_scratch_logical_bytes: usize,
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineCompactCompositionPlanStats {
    pub(crate) const fn composition(self) -> ResidualAffineCompositionPlanStats {
        self.composition
    }

    residual_affine_stats_getters!(
        context_fingerprint_bytes,
        geometry_integer_entries_inspected,
        geometry_integer_bit_work,
        geometry_replay_comparison_work,
        geometry_replay_integer_bit_work,
        geometry_replay_scratch_logical_bytes,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
    );
}

/// Redacted, stable diagnostic manifest for binding an algebra plan beside a
/// proof-bearing authority object.
///
/// The checksums are only diagnostics.  They never establish authority:
/// [`ResidualAffineCompactCompositionPlan::replay`] compares the private
/// context fingerprint and every compact-geometry component exactly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineCompactCompositionManifest {
    schema: &'static str,
    context_fingerprint_bytes: usize,
    context_checksum: u64,
    ambient_arity: usize,
    free_count: usize,
    geometry_checksum: u64,
    limits: ResidualAffineCompactCompositionPlanLimits,
    stats: ResidualAffineCompactCompositionPlanStats,
}

impl ResidualAffineCompactCompositionManifest {
    pub(crate) const fn schema(self) -> &'static str {
        self.schema
    }

    pub(crate) const fn context_fingerprint_bytes(self) -> usize {
        self.context_fingerprint_bytes
    }

    pub(crate) const fn context_checksum(self) -> u64 {
        self.context_checksum
    }

    pub(crate) const fn ambient_arity(self) -> usize {
        self.ambient_arity
    }

    pub(crate) const fn free_count(self) -> usize {
        self.free_count
    }

    pub(crate) const fn geometry_checksum(self) -> u64 {
        self.geometry_checksum
    }

    pub(crate) const fn limits(self) -> ResidualAffineCompactCompositionPlanLimits {
        self.limits
    }

    pub(crate) const fn stats(self) -> ResidualAffineCompactCompositionPlanStats {
        self.stats
    }
}

/// Bounds checked before entering either Symbolica affine-composition
/// backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidualUnitAffinePolynomialCompositionLimits {
    pub exact_algebra: ExactAlgebraLimits,
    pub max_source_terms: usize,
    pub max_source_exponent_entries: usize,
    pub max_expanded_contributions: usize,
    pub max_output_terms: usize,
    pub max_output_exponent_entries: usize,
    pub max_power_calls: usize,
    /// Compatibility-named power-expansion work bound. For the polynomial
    /// evaluator this bounds heap pairs; for expression expansion it remains
    /// the conservative `image_terms * powered_terms` admission policy.
    pub max_native_power_heap_pairs: usize,
    pub max_multiplication_term_pairs: usize,
    /// Compatibility-named structural-work limit. This bounds sparse
    /// addition visits for the polynomial evaluator and literal replacement
    /// plus Atom normalization/conversion visits for expression expansion.
    pub max_addition_term_visits: usize,
    pub max_kronecker_exponent_bits: usize,
    pub max_integer_coefficient_bits: usize,
    pub max_integer_bit_work: usize,
    pub max_normalization_input_term_pairs: usize,
    pub max_guard_origins: usize,
    pub max_guard_origin_retained_bytes: usize,
}

impl Default for ResidualUnitAffinePolynomialCompositionLimits {
    fn default() -> Self {
        Self {
            exact_algebra: ExactAlgebraLimits::default(),
            max_source_terms: 4_000_000,
            max_source_exponent_entries: 268_435_456,
            max_expanded_contributions: 4_000_000,
            max_output_terms: 4_000_000,
            max_output_exponent_entries: 268_435_456,
            max_power_calls: 268_435_456,
            max_native_power_heap_pairs: 536_870_912,
            max_multiplication_term_pairs: 536_870_912,
            max_addition_term_visits: 536_870_912,
            max_kronecker_exponent_bits: 1_000_000,
            max_integer_coefficient_bits: 16_000_000,
            max_integer_bit_work: 1_073_741_824,
            max_normalization_input_term_pairs: 16_000_000,
            max_guard_origins: 65_536,
            max_guard_origin_retained_bytes: 256 * 1024 * 1024,
        }
    }
}

/// Exact prospective and retained work census for one composition.
///
/// Expansion and backend-operation fields are conservative preflight bounds;
/// `output_exponent_entry_bound` is the dense prospective preflight charge,
/// while `output_terms` and `output_exponent_entries` are measured after the
/// selected Symbolica call. Rational normalization has a separate, per-half
/// coefficient census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualUnitAffinePolynomialCompositionStats {
    source_terms: usize,
    source_exponent_entries: usize,
    expanded_contribution_bound: usize,
    output_terms: usize,
    output_exponent_entry_bound: usize,
    output_exponent_entries: usize,
    power_calls: usize,
    native_power_heap_pair_bound: usize,
    multiplication_term_pair_bound: usize,
    addition_term_visit_bound: usize,
    largest_kronecker_exponent_bits: usize,
    largest_integer_coefficient_bit_bound: usize,
    native_integer_bit_work_bound: usize,
    integer_bit_work_bound: usize,
}

impl ResidualUnitAffinePolynomialCompositionStats {
    residual_affine_stats_getters!(
        source_terms,
        source_exponent_entries,
        expanded_contribution_bound,
        output_terms,
        output_exponent_entry_bound,
        output_exponent_entries,
        power_calls,
        multiplication_term_pair_bound,
        largest_kronecker_exponent_bits,
        largest_integer_coefficient_bit_bound,
        integer_bit_work_bound,
    );

    /// Compatibility-named power-expansion census; see
    /// [`ResidualUnitAffinePolynomialCompositionLimits::max_native_power_heap_pairs`].
    pub const fn native_power_heap_pair_bound(self) -> usize {
        self.native_power_heap_pair_bound
    }

    /// Structural-work census for the selected Symbolica backend. The field
    /// name is retained for schema/API compatibility.
    pub const fn addition_term_visit_bound(self) -> usize {
        self.addition_term_visit_bound
    }

    /// Complete pre-output integer-work census for the selected Symbolica
    /// backend. The name is retained for schema/API compatibility.
    pub const fn native_integer_bit_work_bound(self) -> usize {
        self.native_integer_bit_work_bound
    }
}

/// Lossless numerator/denominator work census for one rational coefficient
/// composition.
///
/// Keeping both halves makes replay diagnostics unambiguous: equal aggregate
/// work is not allowed to hide a numerator/denominator workload swap.  The
/// aggregate is retained as a convenience for row-wide budgets and is exactly
/// recomputed with checked sums (largest-bound fields use `max`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ResidualUnitAffineCoefficientCompositionStats {
    numerator: ResidualUnitAffinePolynomialCompositionStats,
    denominator: ResidualUnitAffinePolynomialCompositionStats,
    aggregate: ResidualUnitAffinePolynomialCompositionStats,
    durable_guard_terms: usize,
    durable_guard_exponent_entries: usize,
    durable_guard_integer_bit_payload: usize,
    durable_guard_origin_retained_bytes: usize,
    total_integer_bit_work_bound: usize,
    normalization_input_term_pairs: usize,
}

impl ResidualUnitAffineCoefficientCompositionStats {
    pub const fn numerator(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.numerator
    }

    pub const fn denominator(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.denominator
    }

    pub const fn aggregate(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.aggregate
    }

    pub const fn durable_guard_terms(self) -> usize {
        self.durable_guard_terms
    }

    /// Terms in the durable pre-normalization denominator copy.
    ///
    /// The older unit-affine adapter uses that copy as a guard, hence the
    /// historical field name.  Source-neutral affine composition exposes the
    /// polynomial to its caller for classification and uses the same bounded
    /// storage census without manufacturing provenance.
    pub const fn durable_denominator_terms(self) -> usize {
        self.durable_guard_terms
    }

    pub const fn durable_guard_exponent_entries(self) -> usize {
        self.durable_guard_exponent_entries
    }

    pub const fn durable_denominator_exponent_entries(self) -> usize {
        self.durable_guard_exponent_entries
    }

    pub const fn durable_guard_integer_bit_payload(self) -> usize {
        self.durable_guard_integer_bit_payload
    }

    pub const fn durable_denominator_integer_bit_payload(self) -> usize {
        self.durable_guard_integer_bit_payload
    }

    pub const fn durable_guard_origin_retained_bytes(self) -> usize {
        self.durable_guard_origin_retained_bytes
    }

    pub const fn total_integer_bit_work_bound(self) -> usize {
        self.total_integer_bit_work_bound
    }

    pub const fn normalization_input_term_pairs(self) -> usize {
        self.normalization_input_term_pairs
    }
}

/// Typed failure at the RustRed boundary around Symbolica's simultaneous
/// polynomial/expression composition backends and fraction normalizer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResidualUnitAffineCompositionError {
    SchemaMismatch,
    WrongContext,
    InvalidCompactGeometry {
        reason: &'static str,
    },
    CompactGeometryReplayMismatch,
    WrongArity {
        expected: usize,
        actual: usize,
    },
    NonFreeIndexSurvived {
        position: usize,
    },
    ExponentLimit {
        source_term: usize,
        target_variable: usize,
        requested: u128,
        limit: u128,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    CompositionInvariantViolation {
        resource: &'static str,
        actual: usize,
        bound: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    SymbolicaPanic {
        stage: &'static str,
    },
    Map(ResidualUnitAffineIndexMapError),
    IntegerSystem(ResidualAffineIntegerSystemError),
    Coefficient(ParametricCoefficientError),
}

impl fmt::Display for ResidualUnitAffineCompositionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SchemaMismatch => {
                formatter.write_str("residual unit-affine composition schema mismatch")
            }
            Self::WrongContext => formatter.write_str(
                "residual unit-affine composition belongs to another authenticated K(n) context",
            ),
            Self::InvalidCompactGeometry { reason } => {
                write!(formatter, "invalid compact affine geometry: {reason}")
            }
            Self::CompactGeometryReplayMismatch => formatter
                .write_str("compact affine geometry does not exactly replay the sealed plan"),
            Self::WrongArity { expected, actual } => write!(
                formatter,
                "residual unit-affine composition has arity {actual}, expected {expected}"
            ),
            Self::NonFreeIndexSurvived { position } => write!(
                formatter,
                "non-free index position {position} survived unit-affine composition"
            ),
            Self::ExponentLimit {
                source_term,
                target_variable,
                requested,
                limit,
            } => write!(
                formatter,
                "unit-affine source term {source_term} needs exponent {requested} in target variable {target_variable}, above limit {limit}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "unit-affine {resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "unit-affine {resource} count overflowed usize")
            }
            Self::CompositionInvariantViolation {
                resource,
                actual,
                bound,
            } => write!(
                formatter,
                "unit-affine composition invariant failed for {resource}: retained {actual}, prospective bound {bound}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "unit-affine {resource} could not allocate {requested} bounded entries"
            ),
            Self::SymbolicaPanic { stage } => {
                write!(formatter, "Symbolica panicked during {stage}")
            }
            Self::Map(error) => error.fmt(formatter),
            Self::IntegerSystem(error) => error.fmt(formatter),
            Self::Coefficient(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ResidualUnitAffineCompositionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Map(error) => Some(error),
            Self::IntegerSystem(error) => Some(error),
            Self::Coefficient(error) => Some(error),
            _ => None,
        }
    }
}

impl From<ResidualUnitAffineIndexMapError> for ResidualUnitAffineCompositionError {
    fn from(value: ResidualUnitAffineIndexMapError) -> Self {
        Self::Map(value)
    }
}

impl From<ResidualAffineIntegerSystemError> for ResidualUnitAffineCompositionError {
    fn from(value: ResidualAffineIntegerSystemError) -> Self {
        Self::IntegerSystem(value)
    }
}

impl From<ParametricCoefficientError> for ResidualUnitAffineCompositionError {
    fn from(value: ParametricCoefficientError) -> Self {
        Self::Coefficient(value)
    }
}

#[derive(Clone, Debug)]
struct ResidualAffineCompositionCorePlan {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    ambient_arity: usize,
    free_positions: Vec<usize>,
    nonfree_positions: Vec<usize>,
    linear_support: Vec<bool>,
    full_images: Vec<CoefficientPolynomial>,
    image_term_counts: Vec<usize>,
    image_coefficient_growth_bits: Vec<usize>,
    limits: ResidualUnitAffineCompositionPlanLimits,
    stats: ResidualAffineCompositionPlanStats,
}

impl ResidualAffineCompositionCorePlan {
    fn linear_is_nonzero(&self, row: usize, free_ordinal: usize) -> Option<bool> {
        let offset = row
            .checked_mul(self.free_positions.len())?
            .checked_add(free_ordinal)?;
        self.linear_support.get(offset).copied()
    }

    fn owned_retained_byte_bound(&self) -> Option<usize> {
        let mut bytes = arc_payload_control_and_padding_byte_bound::<Self>()?;
        bytes = bytes.checked_add(
            self.free_positions
                .capacity()
                .checked_mul(size_of::<usize>())?,
        )?;
        bytes = bytes.checked_add(
            self.nonfree_positions
                .capacity()
                .checked_mul(size_of::<usize>())?,
        )?;
        let linear_support_bytes = self
            .linear_support
            .capacity()
            .checked_add(u8::BITS as usize - 1)?
            .checked_div(u8::BITS as usize)?;
        bytes = bytes.checked_add(linear_support_bytes)?;
        bytes = bytes.checked_add(
            self.full_images
                .capacity()
                .checked_mul(size_of::<CoefficientPolynomial>())?,
        )?;
        for image in &self.full_images {
            bytes = bytes.checked_add(polynomial_owned_retained_byte_bound(image)?)?;
        }
        bytes = bytes.checked_add(
            self.image_term_counts
                .capacity()
                .checked_mul(size_of::<usize>())?,
        )?;
        bytes = bytes.checked_add(
            self.image_coefficient_growth_bits
                .capacity()
                .checked_mul(size_of::<usize>())?,
        )?;
        Some(bytes)
    }
}

/// Legacy authenticated wrapper for the compact unit-affine certificate.
///
/// All polynomial work is delegated to the source-neutral core.  Keeping the
/// certificate here prevents the legacy coefficient/condition adapters from
/// losing the provenance needed to construct their historical guard origins.
#[derive(Clone, Debug)]
pub(crate) struct ResidualUnitAffineCompositionPlan {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    map: Arc<ResidualUnitAffineIndexMapCertificate>,
    core: Arc<ResidualAffineCompositionCorePlan>,
    limits: ResidualUnitAffineCompositionPlanLimits,
}

impl ResidualUnitAffineCompositionPlan {
    pub(crate) fn map(&self) -> &Arc<ResidualUnitAffineIndexMapCertificate> {
        &self.map
    }

    pub(crate) const fn limits(&self) -> ResidualUnitAffineCompositionPlanLimits {
        self.limits
    }
}

/// Source-neutral simultaneous affine-composition plan authenticated by a
/// replayed residual integer-system certificate.
#[derive(Clone, Debug)]
pub(crate) struct ResidualAffineCompositionPlan {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    certificate: Arc<ResidualAffineIntegerSystemCertificate>,
    core: Arc<ResidualAffineCompositionCorePlan>,
    limits: ResidualUnitAffineCompositionPlanLimits,
    stats: ResidualAffineCompositionPlanStats,
}

/// Authority-neutral V2 composition plan for one exact compact affine map.
///
/// The plan owns only the canonical Symbolica substitution images and their
/// bounded support metadata.  It deliberately owns neither a generated-case
/// authority nor a residual integer-system certificate.  A proof-bearing
/// caller retains that authority and uses [`Self::replay`] at its boundary.
#[derive(Clone)]
pub(crate) struct ResidualAffineCompactCompositionPlan {
    schema: &'static str,
    context_fingerprint: Arc<str>,
    geometry_checksum: u64,
    core: Arc<ResidualAffineCompositionCorePlan>,
    limits: ResidualAffineCompactCompositionPlanLimits,
    stats: ResidualAffineCompactCompositionPlanStats,
    manifest: ResidualAffineCompactCompositionManifest,
}

impl fmt::Debug for ResidualAffineCompactCompositionPlan {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResidualAffineCompactCompositionPlan")
            .field("manifest", &self.manifest)
            .finish_non_exhaustive()
    }
}

impl ResidualAffineCompactCompositionPlan {
    pub(crate) const fn limits(&self) -> ResidualAffineCompactCompositionPlanLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ResidualAffineCompactCompositionPlanStats {
        self.stats
    }

    pub(crate) const fn manifest(&self) -> ResidualAffineCompactCompositionManifest {
        self.manifest
    }

    pub(crate) fn free_positions(&self) -> &[usize] {
        &self.core.free_positions
    }

    pub(crate) fn ambient_arity(&self) -> usize {
        self.core.ambient_arity
    }

    /// Capacity-sensitive retained allocation bound for diagnostics.  The
    /// allocation-independent exact logical census is stored in `stats` and
    /// was checked before plan construction.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        arc_payload_control_and_padding_byte_bound::<Self>()?
            .checked_add(self.core.owned_retained_byte_bound()?)
    }

    /// Reauthenticate the private context binding and every integer/support
    /// component supplied by a proof-bearing authority.
    pub(crate) fn replay(
        &self,
        context: &ParametricCoefficientContext,
        geometry: ResidualAffineCompactMapView<'_>,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        context.replay_residual_affine_compact_composition_plan(self, geometry)
    }
}

/// Allocation-independent logical memory owned by one source-neutral affine
/// composition plan and the conservative peak reached while constructing it.
///
/// Shared context and integer-system allocations are deliberately excluded.
/// The by-value plan wrapper is charged once, while the plan's uniquely owned
/// `Arc<ResidualAffineCompositionCorePlan>` control block and payload are
/// charged in full. Sparse-vector lengths and actual `Integer::Large` payloads
/// are used; allocator and GMP spare capacity never enters this census.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineCompositionPlanLogicalMemoryCensus {
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
}

impl ResidualAffineCompositionPlanLogicalMemoryCensus {
    pub(crate) const fn retained_owned_logical_bytes(self) -> usize {
        self.retained_owned_logical_bytes
    }

    pub(crate) const fn compilation_owned_logical_peak_upper_bound(self) -> usize {
        self.compilation_owned_logical_peak_upper_bound
    }
}

/// Prospective source-neutral plan memory derived solely from its checked V1
/// limits. This is the parent preflight counterpart of the concrete logical
/// census and deliberately excludes shared context/integer-system payloads.
pub(crate) fn residual_affine_composition_plan_memory_envelope_from_limits(
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<ResidualAffineCompositionPlanLogicalMemoryCensus, ResidualUnitAffineCompositionError> {
    let resource = "affine composition plan memory envelope";
    let variables = limits.max_variables.min(limits.max_full_images);
    let retained_gmp = residual_affine_plan_gmp_logical_bytes_upper_bound(
        limits.max_total_image_terms,
        limits.max_total_image_integer_bits,
    )?;
    let core_arc_control_and_payload =
        arc_payload_control_and_padding_byte_bound::<ResidualAffineCompositionCorePlan>()
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let linear_support_bytes = limits
        .max_support_entries_retained
        .checked_add(u8::BITS as usize - 1)
        .and_then(|bits| bits.checked_div(u8::BITS as usize))
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let retained_owned_logical_bytes = [
        size_of::<ResidualAffineCompositionPlan>(),
        core_arc_control_and_payload,
        residual_affine_checked_mul(resource, variables, size_of::<usize>())?,
        linear_support_bytes,
        residual_affine_checked_mul(resource, variables, size_of::<CoefficientPolynomial>())?,
        residual_affine_checked_mul(resource, limits.max_total_image_terms, size_of::<Integer>())?,
        residual_affine_checked_mul(
            resource,
            limits.max_total_image_exponent_entries,
            size_of::<u16>(),
        )?,
        retained_gmp,
        residual_affine_checked_mul(
            resource,
            residual_affine_checked_mul(resource, 2, variables)?,
            size_of::<usize>(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let compact_geometry_bytes = [
        size_of::<ResidualAffineCompactGeometry>(),
        residual_affine_checked_mul(resource, variables, size_of::<usize>())?,
        residual_affine_checked_mul(
            resource,
            limits.max_geometry_entries_retained,
            size_of::<Integer>(),
        )?,
        residual_affine_plan_gmp_logical_bytes_upper_bound(
            limits.max_geometry_entries_retained,
            limits.max_total_image_integer_bits,
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let exponent_scratch_bytes = residual_affine_checked_add(
        resource,
        size_of::<Vec<u16>>(),
        residual_affine_checked_mul(resource, variables, size_of::<u16>())?,
    )?;
    let largest_image_owned_logical_bytes = [
        size_of::<CoefficientPolynomial>(),
        residual_affine_checked_mul(resource, limits.max_total_image_terms, size_of::<Integer>())?,
        residual_affine_checked_mul(
            resource,
            limits.max_total_image_exponent_entries,
            size_of::<u16>(),
        )?,
        retained_gmp,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let compilation_owned_logical_peak_upper_bound = [
        retained_owned_logical_bytes,
        compact_geometry_bytes,
        exponent_scratch_bytes,
        largest_image_owned_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    Ok(ResidualAffineCompositionPlanLogicalMemoryCensus {
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
    })
}

fn residual_affine_plan_gmp_logical_bytes_upper_bound(
    integer_entries: usize,
    total_integer_bits: usize,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    let resource = "affine composition plan memory envelope";
    let payload_bytes = total_integer_bits / u8::BITS as usize
        + usize::from(total_integer_bits % u8::BITS as usize != 0);
    residual_affine_checked_add(
        resource,
        payload_bytes,
        residual_affine_checked_add(
            resource,
            residual_affine_checked_mul(resource, integer_entries, size_of::<usize>())?,
            integer_entries.saturating_sub(1),
        )?,
    )
}

impl ResidualAffineCompositionPlan {
    pub(crate) fn certificate(&self) -> &Arc<ResidualAffineIntegerSystemCertificate> {
        &self.certificate
    }

    pub(crate) const fn limits(&self) -> ResidualUnitAffineCompositionPlanLimits {
        self.limits
    }

    pub(crate) const fn stats(&self) -> ResidualAffineCompositionPlanStats {
        self.stats
    }

    /// Complete bytes uniquely retained by an `Arc<Self>`, excluding deep
    /// payloads of shared context and integer-system certificates.
    ///
    /// This observes actual vector and GMP limb capacities after plan
    /// construction, complementing the allocation-independent preflight used
    /// by outer transactions.
    pub(crate) fn owned_retained_byte_bound(&self) -> Option<usize> {
        arc_payload_control_and_padding_byte_bound::<Self>()?
            .checked_add(self.core.owned_retained_byte_bound()?)
    }

    /// Recompute the allocation-independent logical-memory census from the
    /// authenticated plan payload. This remains separate from the legacy
    /// capacity-based helper above so V1 accounting and behavior stay frozen.
    pub(crate) fn recompute_logical_memory_census(
        &self,
    ) -> Result<ResidualAffineCompositionPlanLogicalMemoryCensus, ResidualUnitAffineCompositionError>
    {
        residual_affine_composition_plan_logical_memory_census(self)
    }
}

/// Rebuild the source-neutral affine-plan statistics and logical-memory
/// census directly from an already authenticated integer-system affine map.
///
/// This deliberately performs no replay, allocation, or native Symbolica
/// algebra.  It is the adjacent authentication path for owners which consumed
/// the non-clone fresh-plan authorization and then discarded the transient
/// plan: stored plan-memory scalars are never allowed to authenticate
/// themselves.
fn residual_affine_composition_plan_structural_census(
    context: &ParametricCoefficientContext,
    certificate: &ResidualAffineIntegerSystemCertificate,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<
    (
        ResidualAffineCompositionPlanStats,
        ResidualAffineCompositionPlanLogicalMemoryCensus,
    ),
    ResidualUnitAffineCompositionError,
> {
    let resource = "affine composition plan logical memory";
    let ambient_arity = context.index_count();
    if certificate.ambient_arity() != ambient_arity {
        return Err(ResidualUnitAffineCompositionError::WrongArity {
            expected: ambient_arity,
            actual: certificate.ambient_arity(),
        });
    }
    let map = certificate.affine_map().ok_or_else(|| {
        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
            "affine-composition certificate proves an empty system",
        )
    })?;
    if map.ambient_arity() != ambient_arity {
        return Err(ResidualUnitAffineCompositionError::WrongArity {
            expected: ambient_arity,
            actual: map.ambient_arity(),
        });
    }

    let nonfree_positions = map.pivot_positions();
    let free_positions = map.free_positions();
    let support_positions =
        residual_affine_checked_add(resource, nonfree_positions.len(), free_positions.len())?;
    if support_positions != ambient_arity
        || nonfree_positions.windows(2).any(|pair| pair[0] >= pair[1])
        || free_positions.windows(2).any(|pair| pair[0] >= pair[1])
        || nonfree_positions
            .last()
            .is_some_and(|&position| position >= ambient_arity)
        || free_positions
            .last()
            .is_some_and(|&position| position >= ambient_arity)
    {
        return Err(residual_affine_integer_geometry_error(
            "pivot/free positions do not partition the ambient coordinates",
        ));
    }
    for position in 0..ambient_arity {
        let nonfree = nonfree_positions.binary_search(&position).is_ok();
        let free = free_positions.binary_search(&position).is_ok();
        if nonfree == free {
            return Err(residual_affine_integer_geometry_error(
                "pivot/free positions are not a disjoint complete partition",
            ));
        }
    }

    let base_count = context.base.variables().len();
    let variables = residual_affine_checked_add(resource, base_count, ambient_arity)?;
    check_residual_affine_limit("composition variables", variables, limits.max_variables)?;
    check_residual_affine_limit("full-point images", variables, limits.max_full_images)?;

    let linear_support_entries =
        residual_affine_checked_mul(resource, ambient_arity, free_positions.len())?;
    let geometry_entries_retained =
        residual_affine_checked_add(resource, ambient_arity, linear_support_entries)?;
    let geometry_entries_inspected = residual_affine_checked_add(
        resource,
        ambient_arity,
        residual_affine_checked_mul(resource, ambient_arity, ambient_arity)?,
    )?;
    let support_entries_retained = geometry_entries_retained;
    check_residual_affine_limit(
        "affine geometry entries inspected",
        geometry_entries_inspected,
        limits.max_geometry_entries_inspected,
    )?;
    check_residual_affine_limit(
        "affine geometry entries retained",
        geometry_entries_retained,
        limits.max_geometry_entries_retained,
    )?;
    check_residual_affine_limit(
        "affine support entries retained",
        support_entries_retained,
        limits.max_support_entries_retained,
    )?;

    let unit_bits = residual_affine_integer_bits(&Integer::one())?;
    let mut total_image_terms = base_count;
    let mut total_image_integer_bits =
        residual_affine_checked_mul(resource, base_count, unit_bits)?;
    let mut largest_image_integer_bits = usize::from(base_count != 0) * unit_bits;
    let base_image_dynamic_bytes = residual_affine_checked_add(
        resource,
        size_of::<Integer>(),
        residual_affine_checked_mul(resource, variables, size_of::<u16>())?,
    )?;
    let mut full_image_dynamic_bytes =
        residual_affine_checked_mul(resource, base_count, base_image_dynamic_bytes)?;
    let mut largest_image_owned_logical_bytes = if base_count == 0 {
        0
    } else {
        residual_affine_checked_add(
            resource,
            size_of::<CoefficientPolynomial>(),
            base_image_dynamic_bytes,
        )?
    };
    let mut compact_geometry_large_payload_bytes = 0usize;

    for row in 0..ambient_arity {
        let constant = map.constant(row).ok_or_else(|| {
            residual_affine_integer_geometry_error("ambient affine map is missing a constant")
        })?;
        let constant_bits = residual_affine_integer_bits(constant)?;
        largest_image_integer_bits = largest_image_integer_bits.max(constant_bits);
        total_image_integer_bits =
            residual_affine_checked_add(resource, total_image_integer_bits, constant_bits)?;
        let mut term_count = usize::from(!constant.is_zero());
        let mut image_large_payload_bytes = if constant.is_zero() {
            0
        } else {
            residual_affine_large_integer_dynamic_logical_bytes(constant, resource)?
        };

        for column in 0..ambient_arity {
            let coefficient = map.linear_coefficient(row, column).ok_or_else(|| {
                residual_affine_integer_geometry_error(
                    "ambient affine map is missing a square-matrix coefficient",
                )
            })?;
            if nonfree_positions.binary_search(&column).is_ok() {
                if !coefficient.is_zero() {
                    return Err(residual_affine_integer_geometry_error(
                        "ambient affine map has a nonzero nonfree column",
                    ));
                }
                continue;
            }
            let free_ordinal = free_positions.binary_search(&column).map_err(|_| {
                residual_affine_integer_geometry_error(
                    "pivot/free positions are not a disjoint complete partition",
                )
            })?;
            let coefficient_bits = residual_affine_integer_bits(coefficient)?;
            largest_image_integer_bits = largest_image_integer_bits.max(coefficient_bits);
            total_image_integer_bits =
                residual_affine_checked_add(resource, total_image_integer_bits, coefficient_bits)?;
            if row == free_positions[free_ordinal] {
                if coefficient != &Integer::one() {
                    return Err(residual_affine_integer_geometry_error(
                        "free ambient affine row is not an identity row",
                    ));
                }
            } else if free_positions.binary_search(&row).is_ok() && !coefficient.is_zero() {
                return Err(residual_affine_integer_geometry_error(
                    "free ambient affine row is not an identity row",
                ));
            }
            if !coefficient.is_zero() {
                term_count = residual_affine_checked_add(resource, term_count, 1)?;
                image_large_payload_bytes = residual_affine_checked_add(
                    resource,
                    image_large_payload_bytes,
                    residual_affine_large_integer_dynamic_logical_bytes(coefficient, resource)?,
                )?;
            }
        }
        if free_positions.binary_search(&row).is_ok() && !constant.is_zero() {
            return Err(residual_affine_integer_geometry_error(
                "free ambient affine row has nonzero translation",
            ));
        }

        total_image_terms = residual_affine_checked_add(resource, total_image_terms, term_count)?;
        let image_dynamic_bytes = [
            residual_affine_checked_mul(resource, term_count, size_of::<Integer>())?,
            residual_affine_checked_mul(
                resource,
                residual_affine_checked_mul(resource, term_count, variables)?,
                size_of::<u16>(),
            )?,
            image_large_payload_bytes,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| {
            residual_affine_checked_add(resource, sum, bytes)
        })?;
        full_image_dynamic_bytes =
            residual_affine_checked_add(resource, full_image_dynamic_bytes, image_dynamic_bytes)?;
        largest_image_owned_logical_bytes =
            largest_image_owned_logical_bytes.max(residual_affine_checked_add(
                resource,
                size_of::<CoefficientPolynomial>(),
                image_dynamic_bytes,
            )?);
        compact_geometry_large_payload_bytes = residual_affine_checked_add(
            resource,
            compact_geometry_large_payload_bytes,
            image_large_payload_bytes,
        )?;
    }

    check_residual_affine_limit(
        "total image terms",
        total_image_terms,
        limits.max_total_image_terms,
    )?;
    let total_image_exponent_entries =
        residual_affine_checked_mul(resource, total_image_terms, variables)?;
    check_residual_affine_limit(
        "total image exponent entries",
        total_image_exponent_entries,
        limits.max_total_image_exponent_entries,
    )?;
    check_residual_affine_limit(
        "image integer coefficient bits",
        largest_image_integer_bits,
        limits.max_image_integer_bits,
    )?;
    check_residual_affine_limit(
        "total image integer bits",
        total_image_integer_bits,
        limits.max_total_image_integer_bits,
    )?;

    let stats = ResidualAffineCompositionPlanStats {
        variables,
        full_images: variables,
        geometry_entries_inspected,
        geometry_entries_retained,
        support_entries_retained,
        total_image_terms,
        total_image_exponent_entries,
        largest_image_integer_bits,
        total_image_integer_bits,
    };
    let core_arc_control_and_payload =
        arc_payload_control_and_padding_byte_bound::<ResidualAffineCompositionCorePlan>()
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let linear_support_bytes = linear_support_entries
        .checked_add(u8::BITS as usize - 1)
        .and_then(|bits| bits.checked_div(u8::BITS as usize))
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let retained_owned_logical_bytes = [
        size_of::<ResidualAffineCompositionPlan>(),
        core_arc_control_and_payload,
        residual_affine_checked_mul(resource, free_positions.len(), size_of::<usize>())?,
        residual_affine_checked_mul(resource, nonfree_positions.len(), size_of::<usize>())?,
        linear_support_bytes,
        residual_affine_checked_mul(resource, variables, size_of::<CoefficientPolynomial>())?,
        full_image_dynamic_bytes,
        residual_affine_checked_mul(resource, variables, size_of::<usize>())?,
        residual_affine_checked_mul(resource, variables, size_of::<usize>())?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let compact_geometry_bytes = [
        size_of::<ResidualAffineCompactGeometry>(),
        residual_affine_checked_mul(resource, ambient_arity, size_of::<usize>())?,
        residual_affine_checked_mul(resource, geometry_entries_retained, size_of::<Integer>())?,
        compact_geometry_large_payload_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let exponent_scratch_bytes = residual_affine_checked_add(
        resource,
        size_of::<Vec<u16>>(),
        residual_affine_checked_mul(resource, variables, size_of::<u16>())?,
    )?;
    let compilation_owned_logical_peak_upper_bound = [
        retained_owned_logical_bytes,
        compact_geometry_bytes,
        exponent_scratch_bytes,
        largest_image_owned_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    Ok((
        stats,
        ResidualAffineCompositionPlanLogicalMemoryCensus {
            retained_owned_logical_bytes,
            compilation_owned_logical_peak_upper_bound,
        },
    ))
}

fn residual_affine_composition_plan_logical_memory_census(
    plan: &ResidualAffineCompositionPlan,
) -> Result<ResidualAffineCompositionPlanLogicalMemoryCensus, ResidualUnitAffineCompositionError> {
    let resource = "affine composition plan logical memory";
    if plan.schema != RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA
        || plan.core.schema != RESIDUAL_AFFINE_COMPOSITION_CORE_V1_SCHEMA
        || plan.limits != plan.core.limits
        || plan.stats != plan.core.stats
    {
        return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
    }

    let core = &plan.core;
    let variables = core.full_images.len();
    if variables != core.image_term_counts.len()
        || variables != core.image_coefficient_growth_bits.len()
        || variables != core.stats.variables
        || variables != core.stats.full_images
    {
        return Err(
            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                resource: "affine composition plan image metadata",
                actual: variables,
                bound: core.stats.variables,
            },
        );
    }
    let support_positions = residual_affine_checked_add(
        resource,
        core.free_positions.len(),
        core.nonfree_positions.len(),
    )?;
    if support_positions != core.ambient_arity {
        return Err(
            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                resource: "affine composition plan support partition",
                actual: support_positions,
                bound: core.ambient_arity,
            },
        );
    }
    let linear_support_entries =
        residual_affine_checked_mul(resource, core.ambient_arity, core.free_positions.len())?;
    if linear_support_entries != core.linear_support.len() {
        return Err(
            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                resource: "affine composition plan linear support",
                actual: core.linear_support.len(),
                bound: linear_support_entries,
            },
        );
    }
    let geometry_entries_retained =
        residual_affine_checked_add(resource, core.ambient_arity, linear_support_entries)?;
    let geometry_entries_inspected = residual_affine_checked_add(
        resource,
        core.ambient_arity,
        residual_affine_checked_mul(resource, core.ambient_arity, core.ambient_arity)?,
    )?;
    let support_entries_retained =
        residual_affine_checked_add(resource, core.ambient_arity, linear_support_entries)?;

    let mut total_image_terms = 0usize;
    let mut total_image_exponent_entries = 0usize;
    let mut total_image_integer_bits = 0usize;
    let mut largest_image_integer_bits = 0usize;
    let mut full_image_dynamic_bytes = 0usize;
    let mut largest_image_owned_logical_bytes = 0usize;
    let mut compact_geometry_large_payload_bytes = 0usize;
    let base_identity_images = variables.checked_sub(core.ambient_arity).ok_or(
        ResidualUnitAffineCompositionError::CompositionInvariantViolation {
            resource: "affine composition plan base identity images",
            actual: variables,
            bound: core.ambient_arity,
        },
    )?;
    for (ordinal, image) in core.full_images.iter().enumerate() {
        let terms = image.coefficients.len();
        let expected_exponents = residual_affine_checked_mul(resource, terms, variables)?;
        if expected_exponents != image.exponents.len()
            || core.image_term_counts.get(ordinal) != Some(&terms)
        {
            return Err(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "affine composition plan sparse image",
                    actual: image.exponents.len(),
                    bound: expected_exponents,
                },
            );
        }
        if ordinal < base_identity_images {
            if image.coefficients.as_slice() != [Integer::one()] {
                return Err(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "affine composition plan base identity coefficient",
                        actual: terms,
                        bound: 1,
                    },
                );
            }
        } else {
            let row = ordinal - base_identity_images;
            let support_start =
                residual_affine_checked_mul(resource, row, core.free_positions.len())?;
            let support_end =
                residual_affine_checked_add(resource, support_start, core.free_positions.len())?;
            let nonzero_linear_terms = core
                .linear_support
                .get(support_start..support_end)
                .ok_or(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "affine composition plan image support",
                        actual: support_end,
                        bound: core.linear_support.len(),
                    },
                )?
                .iter()
                .filter(|&&present| present)
                .count();
            let maximum_terms = residual_affine_checked_add(resource, nonzero_linear_terms, 1)?;
            if terms < nonzero_linear_terms || terms > maximum_terms {
                return Err(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "affine composition plan image geometry",
                        actual: terms,
                        bound: maximum_terms,
                    },
                );
            }
        }
        total_image_terms = residual_affine_checked_add(resource, total_image_terms, terms)?;
        total_image_exponent_entries = residual_affine_checked_add(
            resource,
            total_image_exponent_entries,
            image.exponents.len(),
        )?;
        let image_dynamic = residual_affine_polynomial_dynamic_logical_bytes(image, resource)?;
        full_image_dynamic_bytes =
            residual_affine_checked_add(resource, full_image_dynamic_bytes, image_dynamic)?;
        largest_image_owned_logical_bytes =
            largest_image_owned_logical_bytes.max(residual_affine_checked_add(
                resource,
                size_of::<CoefficientPolynomial>(),
                image_dynamic,
            )?);
        for coefficient in &image.coefficients {
            let bits = residual_affine_integer_bits(coefficient)?;
            total_image_integer_bits =
                residual_affine_checked_add(resource, total_image_integer_bits, bits)?;
            largest_image_integer_bits = largest_image_integer_bits.max(bits);
            // Every nonzero compact constant/linear coefficient appears once
            // in its integer-system full image: distinct free coordinates have
            // distinct monomials, zero entries own no dynamic payload, and the
            // separately checked base identities are Small `Integer::one()`.
            // The term/support invariant above prevents future image
            // canonicalization from silently invalidating this reconstruction.
            compact_geometry_large_payload_bytes = residual_affine_checked_add(
                resource,
                compact_geometry_large_payload_bytes,
                residual_affine_large_integer_dynamic_logical_bytes(coefficient, resource)?,
            )?;
        }
    }
    if core.stats.geometry_entries_inspected != geometry_entries_inspected
        || core.stats.geometry_entries_retained != geometry_entries_retained
        || core.stats.support_entries_retained != support_entries_retained
        || core.stats.total_image_terms != total_image_terms
        || core.stats.total_image_exponent_entries != total_image_exponent_entries
        || core.stats.total_image_integer_bits != total_image_integer_bits
        || core.stats.largest_image_integer_bits != largest_image_integer_bits
    {
        return Err(
            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                resource: "affine composition plan statistics",
                actual: total_image_terms,
                bound: core.stats.total_image_terms,
            },
        );
    }

    check_residual_affine_limit(
        "composition variables",
        variables,
        plan.limits.max_variables,
    )?;
    check_residual_affine_limit("full-point images", variables, plan.limits.max_full_images)?;
    check_residual_affine_limit(
        "affine geometry entries inspected",
        geometry_entries_inspected,
        plan.limits.max_geometry_entries_inspected,
    )?;
    check_residual_affine_limit(
        "affine geometry entries retained",
        geometry_entries_retained,
        plan.limits.max_geometry_entries_retained,
    )?;
    check_residual_affine_limit(
        "affine support entries retained",
        support_entries_retained,
        plan.limits.max_support_entries_retained,
    )?;
    check_residual_affine_limit(
        "total image terms",
        total_image_terms,
        plan.limits.max_total_image_terms,
    )?;
    check_residual_affine_limit(
        "total image exponent entries",
        total_image_exponent_entries,
        plan.limits.max_total_image_exponent_entries,
    )?;
    check_residual_affine_limit(
        "image integer coefficient bits",
        largest_image_integer_bits,
        plan.limits.max_image_integer_bits,
    )?;
    check_residual_affine_limit(
        "total image integer bits",
        total_image_integer_bits,
        plan.limits.max_total_image_integer_bits,
    )?;

    let core_arc_control_and_payload =
        arc_payload_control_and_padding_byte_bound::<ResidualAffineCompositionCorePlan>()
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let linear_support_bytes = core
        .linear_support
        .len()
        .checked_add(u8::BITS as usize - 1)
        .and_then(|bits| bits.checked_div(u8::BITS as usize))
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let retained_owned_logical_bytes = [
        size_of::<ResidualAffineCompositionPlan>(),
        core_arc_control_and_payload,
        residual_affine_checked_mul(resource, core.free_positions.len(), size_of::<usize>())?,
        residual_affine_checked_mul(resource, core.nonfree_positions.len(), size_of::<usize>())?,
        linear_support_bytes,
        residual_affine_checked_mul(
            resource,
            core.full_images.len(),
            size_of::<CoefficientPolynomial>(),
        )?,
        full_image_dynamic_bytes,
        residual_affine_checked_mul(resource, core.image_term_counts.len(), size_of::<usize>())?,
        residual_affine_checked_mul(
            resource,
            core.image_coefficient_growth_bits.len(),
            size_of::<usize>(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;

    // Compact geometry remains live while support metadata and full-point
    // images are assembled. The final retained plan plus a complete compact
    // geometry, the dense exponent scratch vector, and one current native
    // image therefore gives a conservative, allocation-independent peak.
    let compact_geometry_bytes = [
        size_of::<ResidualAffineCompactGeometry>(),
        residual_affine_checked_mul(resource, support_positions, size_of::<usize>())?,
        residual_affine_checked_mul(resource, geometry_entries_retained, size_of::<Integer>())?,
        compact_geometry_large_payload_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let exponent_scratch_bytes = residual_affine_checked_add(
        resource,
        size_of::<Vec<u16>>(),
        residual_affine_checked_mul(resource, variables, size_of::<u16>())?,
    )?;
    let compilation_owned_logical_peak_upper_bound = [
        retained_owned_logical_bytes,
        compact_geometry_bytes,
        exponent_scratch_bytes,
        largest_image_owned_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;

    Ok(ResidualAffineCompositionPlanLogicalMemoryCensus {
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
    })
}

fn residual_affine_polynomial_dynamic_logical_bytes(
    polynomial: &CoefficientPolynomial,
    resource: &'static str,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    let coefficient_slots = residual_affine_checked_mul(
        resource,
        polynomial.coefficients.len(),
        size_of::<Integer>(),
    )?;
    let exponent_slots =
        residual_affine_checked_mul(resource, polynomial.exponents.len(), size_of::<u16>())?;
    polynomial.coefficients.iter().try_fold(
        residual_affine_checked_add(resource, coefficient_slots, exponent_slots)?,
        |sum, coefficient| {
            residual_affine_checked_add(
                resource,
                sum,
                residual_affine_large_integer_dynamic_logical_bytes(coefficient, resource)?,
            )
        },
    )
}

fn residual_affine_large_integer_dynamic_logical_bytes(
    value: &Integer,
    resource: &'static str,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    let Integer::Large(value) = value else {
        return Ok(0);
    };
    let bits = usize::try_from(value.significant_bits())
        .map_err(|_| ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    bits.checked_add(7)
        .and_then(|bits| bits.checked_div(8))
        .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })
}

#[derive(Clone, Debug)]
pub(crate) struct ResidualAffinePolynomialComposition {
    value: ParametricPolynomial,
    stats: ResidualUnitAffinePolynomialCompositionStats,
}

impl ResidualAffinePolynomialComposition {
    pub(crate) fn value(&self) -> &ParametricPolynomial {
        &self.value
    }

    pub(crate) const fn stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.stats
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ParametricPolynomial,
        ResidualUnitAffinePolynomialCompositionStats,
    ) {
        (self.value, self.stats)
    }
}

pub(crate) type ResidualUnitAffinePolynomialComposition = ResidualAffinePolynomialComposition;

/// Prospective numerator/denominator work for one source-neutral rational
/// composition.  This contains no mapped polynomial and is therefore safe to
/// use for a complete-row resource preflight before either selected Symbolica
/// composition backend is entered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResidualAffineCoefficientCompositionPreflight {
    numerator: ResidualUnitAffinePolynomialCompositionStats,
    denominator: ResidualUnitAffinePolynomialCompositionStats,
    aggregate: ResidualUnitAffinePolynomialCompositionStats,
    durable_denominator_term_bound: usize,
    durable_denominator_exponent_entry_bound: usize,
    durable_denominator_integer_bit_payload_bound: usize,
    normalization_input_term_pair_bound: usize,
    total_integer_bit_work_bound: usize,
}

impl ResidualAffineCoefficientCompositionPreflight {
    pub(crate) const fn numerator(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.numerator
    }

    pub(crate) const fn denominator(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.denominator
    }

    pub(crate) const fn aggregate(self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.aggregate
    }

    pub(crate) const fn durable_denominator_term_bound(self) -> usize {
        self.durable_denominator_term_bound
    }

    pub(crate) const fn durable_denominator_exponent_entry_bound(self) -> usize {
        self.durable_denominator_exponent_entry_bound
    }

    pub(crate) const fn durable_denominator_integer_bit_payload_bound(self) -> usize {
        self.durable_denominator_integer_bit_payload_bound
    }

    pub(crate) const fn normalization_input_term_pair_bound(self) -> usize {
        self.normalization_input_term_pair_bound
    }

    pub(crate) const fn total_integer_bit_work_bound(self) -> usize {
        self.total_integer_bit_work_bound
    }
}

/// Sealed prepared execution for one compact-plan guard composition.
///
/// Every input is borrowed for the token's complete lifetime, so a prepared
/// preflight cannot be replayed against another context, source, or compact
/// plan.  The limits are retained by value and execution consumes the token.
/// Fields stay private so only this module can construct an authenticated
/// prepared execution.
pub(crate) struct PreparedResidualAffineCompactGuardComposition<'prepared> {
    context: &'prepared ParametricCoefficientContext,
    source: &'prepared ParametricPolynomial,
    plan: &'prepared ResidualAffineCompactCompositionPlan,
    limits: ResidualUnitAffinePolynomialCompositionLimits,
    preflight: ResidualUnitAffinePolynomialPreflight,
}

impl PreparedResidualAffineCompactGuardComposition<'_> {
    pub(crate) const fn stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.preflight.stats
    }

    pub(crate) fn execute(
        self,
    ) -> Result<ResidualAffinePolynomialComposition, ResidualUnitAffineCompositionError> {
        self.context.execute_residual_affine_polynomial_core(
            self.source,
            &self.plan.core,
            self.limits,
            self.preflight,
        )
    }
}

/// Sealed prepared execution for one compact-plan rational composition.
///
/// The retained core preflight contains the exact selected-backend numerator
/// and denominator preflights, including the denominator's remaining limit
/// envelope. Execution therefore performs no second source preflight while
/// preserving the existing denominator-before-normalization result.
pub(crate) struct PreparedResidualAffineCompactCoefficientComposition<'prepared> {
    context: &'prepared ParametricCoefficientContext,
    source: &'prepared ParametricCoefficient,
    plan: &'prepared ResidualAffineCompactCompositionPlan,
    limits: ResidualUnitAffinePolynomialCompositionLimits,
    preflight: ResidualAffineCoefficientCorePreflight,
}

impl PreparedResidualAffineCompactCoefficientComposition<'_> {
    pub(crate) const fn stats(&self) -> ResidualAffineCoefficientCompositionPreflight {
        self.preflight.stats
    }

    pub(crate) fn execute(
        self,
    ) -> Result<ResidualAffineCoefficientComposition, ResidualUnitAffineCompositionError> {
        self.context
            .execute_prepared_coefficient_on_residual_affine_core(
                self.source,
                &self.plan.core,
                self.limits,
                self.preflight,
            )
    }
}

/// One source-neutral rational coefficient composed through an authenticated
/// residual affine plan.
///
/// `mapped_denominator` is the exact denominator *before* Symbolica's fraction
/// normalization.  It remains available even when normalization cancels it,
/// the numerator is zero, or it is a nonzero integer constant.  The caller is
/// responsible for classifying that polynomial in its own proof-bearing
/// domain and for attaching any required provenance.
#[derive(Clone, Debug)]
pub(crate) struct ResidualAffineComposedCoefficient {
    value: ParametricCoefficient,
    mapped_denominator: ParametricPolynomial,
    stats: ResidualUnitAffineCoefficientCompositionStats,
}

impl ResidualAffineComposedCoefficient {
    pub(crate) fn value(&self) -> &ParametricCoefficient {
        &self.value
    }

    pub(crate) fn mapped_denominator(&self) -> &ParametricPolynomial {
        &self.mapped_denominator
    }

    pub(crate) const fn stats(&self) -> ResidualUnitAffineCoefficientCompositionStats {
        self.stats
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ParametricCoefficient,
        ParametricPolynomial,
        ResidualUnitAffineCoefficientCompositionStats,
    ) {
        (self.value, self.mapped_denominator, self.stats)
    }
}

/// Source-neutral rational composition outcome.
///
/// A denominator which maps identically to zero is a semantic row-domain
/// outcome, not an algebra or backend failure.  Its complete work census is
/// retained so enclosing replay certificates can account for the attempted
/// row exactly.
#[derive(Clone, Debug)]
pub(crate) enum ResidualAffineCoefficientComposition {
    Available(ResidualAffineComposedCoefficient),
    ZeroMappedDenominator {
        stats: ResidualUnitAffineCoefficientCompositionStats,
    },
}

impl ResidualAffineCoefficientComposition {
    pub(crate) const fn stats(&self) -> ResidualUnitAffineCoefficientCompositionStats {
        match self {
            Self::Available(value) => value.stats,
            Self::ZeroMappedDenominator { stats } => *stats,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct GuardedResidualUnitAffineCoefficientComposition {
    value: ParametricCoefficient,
    mapped_denominator: Option<ParametricNonZeroCondition>,
    stats: ResidualUnitAffineCoefficientCompositionStats,
}

impl GuardedResidualUnitAffineCoefficientComposition {
    pub(crate) fn value(&self) -> &ParametricCoefficient {
        &self.value
    }

    pub(crate) fn mapped_denominator(&self) -> Option<&ParametricNonZeroCondition> {
        self.mapped_denominator.as_ref()
    }

    pub(crate) const fn stats(&self) -> ResidualUnitAffineCoefficientCompositionStats {
        self.stats
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ParametricCoefficient,
        Option<ParametricNonZeroCondition>,
        ResidualUnitAffineCoefficientCompositionStats,
    ) {
        (self.value, self.mapped_denominator, self.stats)
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ResidualUnitAffineCoefficientComposition {
    Available(GuardedResidualUnitAffineCoefficientComposition),
    ZeroMappedDenominator {
        stats: ResidualUnitAffineCoefficientCompositionStats,
    },
}

impl ResidualUnitAffineCoefficientComposition {
    pub(crate) const fn stats(&self) -> ResidualUnitAffineCoefficientCompositionStats {
        match self {
            Self::Available(value) => value.stats,
            Self::ZeroMappedDenominator { stats } => *stats,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) enum ResidualUnitAffineConditionClass {
    Unsatisfiable,
    NonzeroIntegerConstant,
    BaseAssumption(ParametricNonZeroCondition),
    IndexDependent(ParametricNonZeroCondition),
}

#[derive(Clone, Debug)]
pub(crate) struct ResidualUnitAffineConditionComposition {
    class: ResidualUnitAffineConditionClass,
    stats: ResidualUnitAffinePolynomialCompositionStats,
    guard_origin_retained_bytes: usize,
}

impl ResidualUnitAffineConditionComposition {
    pub(crate) fn class(&self) -> &ResidualUnitAffineConditionClass {
        &self.class
    }

    pub(crate) const fn stats(&self) -> ResidualUnitAffinePolynomialCompositionStats {
        self.stats
    }

    pub(crate) const fn guard_origin_retained_bytes(&self) -> usize {
        self.guard_origin_retained_bytes
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        ResidualUnitAffineConditionClass,
        ResidualUnitAffinePolynomialCompositionStats,
        usize,
    ) {
        (self.class, self.stats, self.guard_origin_retained_bytes)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParametricCoefficientError {
    EmptyIndexSpace,
    InvalidScope(String),
    IndexSymbolCollision {
        position: usize,
    },
    WrongContext,
    WrongIndexArity {
        expected: usize,
        actual: usize,
    },
    InvalidIndexPermutation,
    IndexAssignmentOutOfRange {
        position: usize,
        arity: usize,
    },
    DuplicateIndexAssignment {
        position: usize,
    },
    ZeroPolynomialCondition,
    ZeroDenominator,
    DivisionByZero,
    MalformedPolynomial {
        terms: usize,
        exponents: usize,
        variables: usize,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    ResourceCountOverflow {
        resource: &'static str,
    },
    MissingGuardOrigin,
    ExactAlgebra(ExactAlgebraError),
    Symbolica(String),
}

impl fmt::Display for ParametricCoefficientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIndexSpace => {
                formatter.write_str("a parametric context needs at least one index")
            }
            Self::InvalidScope(scope) => {
                write!(formatter, "invalid parametric context scope {scope:?}")
            }
            Self::IndexSymbolCollision { position } => write!(
                formatter,
                "generated parametric index symbol {position} collides with a base variable"
            ),
            Self::WrongContext => formatter.write_str(
                "coefficient or polynomial belongs to a different authenticated context",
            ),
            Self::WrongIndexArity { expected, actual } => write!(
                formatter,
                "index vector has arity {actual}, expected {expected}"
            ),
            Self::InvalidIndexPermutation => formatter.write_str(
                "index-variable transport needs a bijection of the authenticated index space",
            ),
            Self::IndexAssignmentOutOfRange { position, arity } => write!(
                formatter,
                "partial index assignment position {position} is outside arity {arity}"
            ),
            Self::DuplicateIndexAssignment { position } => write!(
                formatter,
                "partial index assignment repeats position {position}"
            ),
            Self::ZeroPolynomialCondition => {
                formatter.write_str("a required nonzero polynomial is identically zero")
            }
            Self::ZeroDenominator => {
                formatter.write_str("rational coefficient has a zero denominator")
            }
            Self::DivisionByZero => {
                formatter.write_str("attempted to divide by an identically zero coefficient")
            }
            Self::MalformedPolynomial {
                terms,
                exponents,
                variables,
            } => write!(
                formatter,
                "polynomial has {terms} terms, {exponents} exponents, and {variables} variables"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "{resource} needs {requested} units, exceeding the configured limit {limit}"
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(formatter, "{resource} count overflowed usize")
            }
            Self::MissingGuardOrigin => {
                formatter.write_str("a nonzero condition needs at least one typed origin")
            }
            Self::ExactAlgebra(error) => error.fmt(formatter),
            Self::Symbolica(message) => formatter.write_str(message),
        }
    }
}

impl std::error::Error for ParametricCoefficientError {}

impl From<ExactAlgebraError> for ParametricCoefficientError {
    fn from(value: ExactAlgebraError) -> Self {
        Self::ExactAlgebra(value)
    }
}

/// Successful specialization of one `K(n)` coefficient back into `K`.
///
/// `nonzero` retains the mapped original denominator before Symbolica can
/// cancel factors in the resulting fraction-field element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedCoefficientSpecialization {
    pub value: Coefficient,
    pub nonzero: Vec<BasePolynomial>,
    guarded_nonzero: Vec<SpecializedNonZeroCondition>,
}

impl GuardedCoefficientSpecialization {
    /// Provenance-preserving view of [`Self::nonzero`].
    pub fn guarded_nonzero_conditions(&self) -> &[SpecializedNonZeroCondition] {
        &self.guarded_nonzero
    }
}

/// A coefficient kept in `K(n)` after only a sparse equality locus is
/// imposed.  The mapped original denominator remains explicit even if
/// fraction normalization cancels it from [`Self::value`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardedPartialCoefficientSpecialization {
    pub value: ParametricCoefficient,
    pub nonzero: Vec<ParametricPolynomial>,
    assignment: PartialIndexAssignment,
    guarded_nonzero: Vec<ParametricNonZeroCondition>,
    stats: PartialPolynomialSpecializationStats,
}

impl GuardedPartialCoefficientSpecialization {
    pub fn assignment(&self) -> &PartialIndexAssignment {
        &self.assignment
    }

    pub fn guarded_nonzero_conditions(&self) -> &[ParametricNonZeroCondition] {
        &self.guarded_nonzero
    }

    pub(crate) fn stats(&self) -> PartialPolynomialSpecializationStats {
        self.stats
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PartialPolynomialSpecializationStats {
    pub source_terms: usize,
    pub output_terms: usize,
    pub power_operations: usize,
    pub integer_bit_bound: usize,
}

/// One exact pair of authenticated fields `K` and `K(n)`.
#[derive(Clone, Debug)]
pub struct ParametricCoefficientContext {
    base: CoefficientContext,
    base_fingerprint: Arc<str>,
    fingerprint: Arc<str>,
    variables: Arc<Vec<PolyVariable>>,
    index_variables: Arc<Vec<PolyVariable>>,
    template: Coefficient,
}

impl ParametricCoefficientContext {
    /// Extend `base` by `index_count` private index variables.
    ///
    /// `scope` is persisted as part of the context identity.  Its bytes are
    /// encoded losslessly in Symbolica's namespace, so two different scopes
    /// cannot alias merely because they sanitize to the same identifier.
    pub fn try_new(
        base: &CoefficientContext,
        scope: &str,
        index_count: usize,
    ) -> Result<Self, ParametricCoefficientError> {
        if index_count == 0 {
            return Err(ParametricCoefficientError::EmptyIndexSpace);
        }
        if scope.is_empty() {
            return Err(ParametricCoefficientError::InvalidScope(scope.to_owned()));
        }

        let encoded_scope = encode_symbol_component(scope.as_bytes());
        let mut index_variables = Vec::with_capacity(index_count);
        for position in 0..index_count {
            let qualified = format!("rustred::parametric_s{encoded_scope}::n{position}");
            let namespaced = NamespacedSymbol::try_parse(&qualified)
                .ok_or_else(|| ParametricCoefficientError::InvalidScope(scope.to_owned()))?;
            let symbol = SymbolBuilder::new(namespaced)
                .build()
                .map_err(|error| ParametricCoefficientError::Symbolica(error.to_string()))?;
            let variable = PolyVariable::Symbol(symbol);
            if base.variables().contains(&variable) {
                return Err(ParametricCoefficientError::IndexSymbolCollision { position });
            }
            index_variables.push(variable);
        }

        let mut variables = Vec::with_capacity(base.variables().len() + index_count);
        variables.extend(base.variables().iter().cloned());
        variables.extend(index_variables.iter().cloned());
        let variables = Arc::new(variables);
        let template = RationalPolynomial::new(&Z, variables.clone());
        let base_fingerprint: Arc<str> = base_context_fingerprint(base).into();
        let fingerprint: Arc<str> = format!(
            "rustred-parametric-context-v1|base={}|scope={}:{}|indices={index_count}",
            base_fingerprint,
            scope.len(),
            scope
        )
        .into();

        Ok(Self {
            base: base.clone(),
            base_fingerprint,
            fingerprint,
            variables,
            index_variables: Arc::new(index_variables),
            template,
        })
    }

    pub fn base(&self) -> &CoefficientContext {
        &self.base
    }

    /// Conservative bytes owned by a deep clone of this paired coefficient
    /// context. String and variable-map payloads stay shared through `Arc`;
    /// both the base-context template and the index-extended template are
    /// deep-cloned and therefore charged.
    pub(crate) fn clone_owned_retained_byte_bound(&self) -> Option<usize> {
        let base = self.base.clone_owned_retained_byte_bound()?;
        let base_template_heap = base.checked_sub(size_of::<CoefficientContext>())?;
        size_of::<Self>()
            .checked_add(base_template_heap)?
            .checked_add(coefficient_owned_retained_byte_bound(&self.template)?)
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    pub fn index_count(&self) -> usize {
        self.index_variables.len()
    }

    pub fn contains(&self, value: &ParametricCoefficient) -> bool {
        value.context.as_ref() == self.fingerprint.as_ref()
            && validate_coefficient_on_map(
                &value.raw,
                &self.variables,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn contains_polynomial(&self, value: &ParametricPolynomial) -> bool {
        value.context.as_ref() == self.fingerprint.as_ref()
            && validate_polynomial_on_map(
                &value.raw,
                &self.variables,
                crate::CoefficientPolynomialPart::Numerator,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn contains_nonzero_condition(&self, value: &ParametricNonZeroCondition) -> bool {
        !value.origins.is_empty() && self.contains_polynomial(&value.polynomial)
    }

    /// Authenticate one polynomial condition and attach one atomic origin.
    pub fn nonzero_condition(
        &self,
        polynomial: ParametricPolynomial,
        origin: GuardOrigin,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.nonzero_condition_with_origins_and_limits(
            polynomial,
            [origin],
            ExactAlgebraLimits::default(),
        )
    }

    /// Authenticate one polynomial condition with an already collected
    /// deterministic origin set.
    ///
    /// The iterator is consumed under the default provenance budget, so an
    /// untrusted or unbounded iterator cannot allocate an unbounded set.  Use
    /// [`Self::nonzero_condition_with_origins_and_origin_limit`] when a caller
    /// needs a stricter budget.
    pub fn nonzero_condition_with_origins_and_limits(
        &self,
        polynomial: ParametricPolynomial,
        origins: impl IntoIterator<Item = GuardOrigin>,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.nonzero_condition_with_origins_and_origin_limit(
            polynomial,
            origins,
            limits,
            ParametricArithmeticLimits::default().max_guard_origins,
        )
    }

    /// Authenticate a condition under independent exact-algebra and
    /// provenance-cardinality budgets.
    pub fn nonzero_condition_with_origins_and_origin_limit(
        &self,
        polynomial: ParametricPolynomial,
        origins: impl IntoIterator<Item = GuardOrigin>,
        limits: ExactAlgebraLimits,
        max_guard_origins: usize,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(&polynomial, limits)?;
        let origins = collect_guard_origins_with_limit(origins, max_guard_origins)?;
        if origins.is_empty() {
            return Err(ParametricCoefficientError::MissingGuardOrigin);
        }
        Ok(ParametricNonZeroCondition {
            polynomial,
            origins,
        })
    }

    /// Retain an origin set which a private proof compiler has already
    /// collected under a prospective allocation envelope.
    ///
    /// Unlike the iterator-facing constructor, this does not rebuild a
    /// second `BTreeSet` while the first tree is still being consumed.  The
    /// polynomial and complete set are nevertheless authenticated at this
    /// boundary, so callers cannot use the allocation seam to bypass context
    /// or provenance invariants.
    pub(crate) fn nonzero_condition_from_prevalidated_parts(
        &self,
        polynomial: ParametricPolynomial,
        origins: BTreeSet<GuardOrigin>,
        exact_algebra: ExactAlgebraLimits,
        max_guard_origins: usize,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(&polynomial, exact_algebra)?;
        if origins.is_empty() {
            return Err(ParametricCoefficientError::MissingGuardOrigin);
        }
        check_limit("parametric guard origins", origins.len(), max_guard_origins)?;
        Ok(ParametricNonZeroCondition {
            polynomial,
            origins,
        })
    }

    pub fn validate_polynomial_with_limits(
        &self,
        value: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ParametricCoefficientError> {
        if value.context.as_ref() != self.fingerprint.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        validate_polynomial_on_map(
            &value.raw,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits,
        )?;
        Ok(())
    }

    /// Return whether an authenticated polynomial depends on at least one of
    /// this context's private denominator-index variables.
    ///
    /// A polynomial involving only base variables is a coefficient in
    /// `K = Q(theta)`, hence a constant with respect to `K[n]` even when its
    /// printed expression is not an integer constant.  Symbolic case splitters
    /// use this distinction to avoid manufacturing an impossible generic-
    /// kinematics branch such as `theta = 0` inside the coefficient field.
    pub fn polynomial_depends_on_indices_with_limits(
        &self,
        value: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<bool, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits)?;
        let first_index = self.base.variables().len();
        Ok(value.raw.exponents_iter().any(|exponents| {
            exponents[first_index..]
                .iter()
                .any(|&exponent| exponent != 0)
        }))
    }

    pub fn polynomial_depends_on_indices(
        &self,
        value: &ParametricPolynomial,
    ) -> Result<bool, ParametricCoefficientError> {
        self.polynomial_depends_on_indices_with_limits(value, ExactAlgebraLimits::default())
    }

    /// Prove the two polynomial zero loci equal up to a unit of the formal
    /// coefficient field `K = Q(theta)`.
    ///
    /// This deliberately proves only the strict associate relation
    /// `left/right in K*`.  It does not infer radical-ideal equivalence.  Both
    /// inputs are authenticated before the bounded projective coefficient
    /// comparison, and a zero input is never an associate.
    pub fn polynomial_loci_are_associates_with_limits(
        &self,
        left: &ParametricPolynomial,
        right: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<bool, ParametricCoefficientError> {
        let associate_limits = ParametricPolynomialAssociateLimits {
            exact_algebra: limits,
            ..ParametricPolynomialAssociateLimits::default()
        };
        Ok(self
            .polynomial_loci_are_associates_with_census(left, right, associate_limits)?
            .associated())
    }

    /// Instrumented associate proof for aggregate outer compilers.
    ///
    /// Regard each input as a coefficient vector over the index monomials,
    /// with entries in `Z[theta]`.  Two nonzero vectors are associates over
    /// `K = Q(theta)` exactly when their index supports agree and every entry
    /// is proportional to one deterministic anchor entry.  Cross products are
    /// compared with bounded reference sorts and fixed-size integer scratch;
    /// no rational normalization, polynomial GCD, or FORM call is involved.
    /// Indeed, if `P_a Q_0 = Q_a P_0` for every index monomial `a`, then
    /// `left = (P_0/Q_0) right`; the converse follows by clearing the same
    /// nonzero coefficient-field unit.
    pub(crate) fn polynomial_loci_are_associates_with_census(
        &self,
        left: &ParametricPolynomial,
        right: &ParametricPolynomial,
        limits: ParametricPolynomialAssociateLimits,
    ) -> Result<ParametricPolynomialAssociateResult, ParametricCoefficientError> {
        let variable_count = self.variables.len();
        let base_variables = self.base.variables().len();
        let index_variables = variable_count.checked_sub(base_variables).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "polynomial-associate index variables",
            },
        )?;
        let validation_terms = checked_parametric_add(
            "polynomial-associate validation terms",
            left.raw.coefficients.len(),
            right.raw.coefficients.len(),
        )?;
        let validation_exponent_entries = checked_parametric_add(
            "polynomial-associate validation exponent entries",
            left.raw.exponents.len(),
            right.raw.exponents.len(),
        )?;
        let mut stats = ParametricPolynomialAssociateStats {
            context_fingerprint_comparison_bytes: checked_parametric_add(
                "polynomial-associate context fingerprint comparison bytes",
                checked_parametric_add(
                    "polynomial-associate context fingerprint comparison bytes",
                    left.authenticated_context_fingerprint().len(),
                    self.fingerprint.len(),
                )?,
                checked_parametric_add(
                    "polynomial-associate context fingerprint comparison bytes",
                    right.authenticated_context_fingerprint().len(),
                    self.fingerprint.len(),
                )?,
            )?,
            variable_map_entry_comparisons: checked_parametric_mul(
                "polynomial-associate variable-map entry comparisons",
                variable_count,
                2,
            )?,
            validation_terms,
            validation_exponent_entries,
            ..ParametricPolynomialAssociateStats::default()
        };

        // Structural counts and comparison envelopes are admitted before an
        // input coefficient or exponent payload is scanned.
        check_associate_stats(&stats, limits)?;
        self.validate_polynomial_with_limits(left, limits.exact_algebra)?;
        self.validate_polynomial_with_limits(right, limits.exact_algebra)?;

        for coefficient in left.raw.coefficients.iter().chain(&right.raw.coefficients) {
            stats.validation_integer_bits = checked_parametric_add(
                "polynomial-associate validation integer bits",
                stats.validation_integer_bits,
                associate_integer_bit_count(coefficient)?,
            )?;
            check_limit(
                "polynomial-associate validation integer bits",
                stats.validation_integer_bits,
                limits.max_validation_integer_bits,
            )?;
        }
        if left.is_zero() || right.is_zero() {
            return Ok(ParametricPolynomialAssociateResult {
                associated: false,
                stats,
            });
        }

        let p_terms = left.raw.nterms();
        let q_terms = right.raw.nterms();
        stats.term_order_entries =
            checked_parametric_add("polynomial-associate term-order entries", p_terms, q_terms)?;
        let p_order_bound = associate_heap_comparison_bound(p_terms)?;
        let q_order_bound = associate_heap_comparison_bound(q_terms)?;
        stats.ordering_comparison_bound = checked_parametric_add(
            "polynomial-associate ordering comparison bound",
            p_order_bound,
            q_order_bound,
        )?;
        let adjacent_scans = checked_parametric_mul(
            "polynomial-associate index-coordinate inspection bound",
            2,
            checked_parametric_add(
                "polynomial-associate index-coordinate inspection bound",
                p_terms.saturating_sub(1),
                q_terms.saturating_sub(1),
            )?,
        )?;
        stats.index_coordinate_inspection_bound = checked_parametric_mul(
            "polynomial-associate index-coordinate inspection bound",
            index_variables,
            checked_parametric_add(
                "polynomial-associate index-coordinate inspection bound",
                stats.ordering_comparison_bound,
                adjacent_scans,
            )?,
        )?;
        let fixed_headers = checked_parametric_mul(
            "polynomial-associate visible temporary byte envelope",
            9,
            size_of::<Vec<usize>>(),
        )?;
        let order_envelope =
            associate_capacity_byte_envelope(stats.term_order_entries, size_of::<usize>())?;
        stats.visible_temporary_byte_envelope = checked_parametric_add(
            "polynomial-associate visible temporary byte envelope",
            fixed_headers,
            order_envelope,
        )?;
        check_associate_stats(&stats, limits)?;

        let mut p_order = Vec::new();
        p_order.try_reserve_exact(p_terms).map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "allocation failed for polynomial-associate left term order".to_owned(),
            )
        })?;
        p_order.extend(0..p_terms);
        associate_check_capacity(
            p_order.capacity(),
            p_terms,
            "polynomial-associate left term-order capacity",
        )?;
        let mut q_order = Vec::new();
        q_order.try_reserve_exact(q_terms).map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "allocation failed for polynomial-associate right term order".to_owned(),
            )
        })?;
        q_order.extend(0..q_terms);
        associate_check_capacity(
            q_order.capacity(),
            q_terms,
            "polynomial-associate right term-order capacity",
        )?;

        let mut observed = AssociateObservedWork::default();
        observed.ordering_comparisons = checked_parametric_add(
            "polynomial-associate observed ordering comparisons",
            associate_heap_sort_by(
                &mut p_order,
                |left_term, right_term| {
                    associate_term_index_cmp(&left.raw, *left_term, *right_term, base_variables)
                },
                p_order_bound,
            )?,
            associate_heap_sort_by(
                &mut q_order,
                |left_term, right_term| {
                    associate_term_index_cmp(&right.raw, *left_term, *right_term, base_variables)
                },
                q_order_bound,
            )?,
        )?;

        // Count once so the group vectors can be admitted and reserved at
        // their exact requested lengths, then scan a second time to populate.
        let p_group_count = associate_count_groups(&left.raw, &p_order, base_variables);
        let q_group_count = associate_count_groups(&right.raw, &q_order, base_variables);
        stats.index_groups = checked_parametric_add(
            "polynomial-associate index groups",
            p_group_count,
            q_group_count,
        )?;
        let group_envelope =
            associate_capacity_byte_envelope(stats.index_groups, size_of::<AssociateIndexGroup>())?;
        stats.visible_temporary_byte_envelope = associate_sum_counts(
            "polynomial-associate visible temporary byte envelope",
            [fixed_headers, order_envelope, group_envelope],
        )?;
        check_associate_stats(&stats, limits)?;
        let p_groups = associate_build_groups(&left.raw, &p_order, base_variables, p_group_count)?;
        let q_groups = associate_build_groups(&right.raw, &q_order, base_variables, q_group_count)?;
        associate_check_capacity(
            p_groups.capacity(),
            p_group_count,
            "polynomial-associate left index-group capacity",
        )?;
        associate_check_capacity(
            q_groups.capacity(),
            q_group_count,
            "polynomial-associate right index-group capacity",
        )?;

        if p_group_count != q_group_count {
            stats.visible_temporary_bytes = associate_sum_counts(
                "polynomial-associate visible temporary bytes",
                [
                    fixed_headers,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        p_order.capacity(),
                        size_of::<usize>(),
                    )?,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        q_order.capacity(),
                        size_of::<usize>(),
                    )?,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        p_groups.capacity(),
                        size_of::<AssociateIndexGroup>(),
                    )?,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        q_groups.capacity(),
                        size_of::<AssociateIndexGroup>(),
                    )?,
                ],
            )?;
            return Ok(ParametricPolynomialAssociateResult {
                associated: false,
                stats,
            });
        }

        stats.index_coordinate_inspection_bound = checked_parametric_add(
            "polynomial-associate index-coordinate inspection bound",
            stats.index_coordinate_inspection_bound,
            checked_parametric_mul(
                "polynomial-associate index-coordinate inspection bound",
                index_variables,
                p_group_count,
            )?,
        )?;
        check_associate_stats(&stats, limits)?;
        let mut support_equal = true;
        for ordinal in 0..p_group_count {
            let p_term = p_order[p_groups[ordinal].begin];
            let q_term = q_order[q_groups[ordinal].begin];
            support_equal &=
                associate_same_index_key(&left.raw, p_term, &right.raw, q_term, base_variables);
        }
        if !support_equal || p_group_count == 1 {
            stats.visible_temporary_bytes = associate_sum_counts(
                "polynomial-associate visible temporary bytes",
                [
                    fixed_headers,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        p_order.capacity(),
                        size_of::<usize>(),
                    )?,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        q_order.capacity(),
                        size_of::<usize>(),
                    )?,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        p_groups.capacity(),
                        size_of::<AssociateIndexGroup>(),
                    )?,
                    checked_parametric_mul(
                        "polynomial-associate visible temporary bytes",
                        q_groups.capacity(),
                        size_of::<AssociateIndexGroup>(),
                    )?,
                ],
            )?;
            return Ok(ParametricPolynomialAssociateResult {
                associated: support_equal,
                stats,
            });
        }

        stats.anchor_cost_operations = checked_parametric_mul(
            "polynomial-associate anchor cost operations",
            p_group_count,
            5,
        )?;
        // The anchor scan is linear in the number of source index groups.
        // Admit that source-structural work before entering the scan.
        check_limit(
            "polynomial-associate anchor cost operations",
            stats.anchor_cost_operations,
            limits.max_anchor_cost_operations,
        )?;
        let mut anchor = 0usize;
        let mut anchor_cost = usize::MAX;
        for ordinal in 0..p_group_count {
            let p_len = p_groups[ordinal].len();
            let q_len = q_groups[ordinal].len();
            let cost = checked_parametric_add(
                "polynomial-associate cross terms",
                checked_parametric_mul(
                    "polynomial-associate cross terms",
                    q_len,
                    p_terms.checked_sub(p_len).ok_or(
                        ParametricCoefficientError::ResourceCountOverflow {
                            resource: "polynomial-associate cross terms",
                        },
                    )?,
                )?,
                checked_parametric_mul(
                    "polynomial-associate cross terms",
                    p_len,
                    q_terms.checked_sub(q_len).ok_or(
                        ParametricCoefficientError::ResourceCountOverflow {
                            resource: "polynomial-associate cross terms",
                        },
                    )?,
                )?,
            )?;
            if cost < anchor_cost {
                anchor = ordinal;
                anchor_cost = cost;
            }
        }
        stats.cross_terms = anchor_cost;
        stats.integer_preflight_pairs = anchor_cost;
        check_limit(
            "polynomial-associate cross terms",
            stats.cross_terms,
            limits.max_cross_terms,
        )?;
        check_limit(
            "polynomial-associate integer preflight pairs",
            stats.integer_preflight_pairs,
            limits.max_integer_preflight_pairs,
        )?;
        check_limit(
            "polynomial-associate cross terms",
            stats.cross_terms,
            limits.exact_algebra.max_term_operations,
        )?;

        stats.magnitude_copy_terms = stats.term_order_entries;
        check_associate_stats(&stats, limits)?;
        for coefficient in left.raw.coefficients.iter().chain(&right.raw.coefficients) {
            stats.magnitude_copy_bytes = checked_parametric_add(
                "polynomial-associate magnitude copy bytes",
                stats.magnitude_copy_bytes,
                associate_bits_to_units(
                    "polynomial-associate magnitude copy bytes",
                    associate_integer_bit_count(coefficient)?,
                    8,
                )?,
            )?;
            check_limit(
                "polynomial-associate magnitude copy bytes",
                stats.magnitude_copy_bytes,
                limits.max_magnitude_copy_bytes,
            )?;
        }

        let p_anchor = p_groups[anchor];
        let q_anchor = q_groups[anchor];
        let mut counted_cross_terms = 0usize;

        // Complete the source-structural group census first.  In particular,
        // no Cartesian coefficient pair is inspected until the peak cross
        // width and every cross-key ordering/coordinate envelope have been
        // admitted.
        for ordinal in 0..p_group_count {
            if ordinal == anchor {
                continue;
            }
            let p_group = p_groups[ordinal];
            let q_group = q_groups[ordinal];
            let cross_terms = associate_cross_term_count(p_group, q_group, p_anchor, q_anchor)?;
            counted_cross_terms = checked_parametric_add(
                "polynomial-associate cross terms",
                counted_cross_terms,
                cross_terms,
            )?;
            stats.peak_cross_terms = stats.peak_cross_terms.max(cross_terms);
            check_limit(
                "polynomial-associate peak cross terms",
                stats.peak_cross_terms,
                limits.max_peak_cross_terms,
            )?;
            let cross_order_bound = associate_heap_comparison_bound(cross_terms)?;
            stats.ordering_comparison_bound = checked_parametric_add(
                "polynomial-associate ordering comparison bound",
                stats.ordering_comparison_bound,
                cross_order_bound,
            )?;
            check_limit(
                "polynomial-associate ordering comparison bound",
                stats.ordering_comparison_bound,
                limits.max_ordering_comparison_bound,
            )?;
            let cross_key_checks = checked_parametric_add(
                "polynomial-associate base-sum coordinate inspection bound",
                cross_order_bound,
                cross_terms.saturating_sub(1),
            )?;
            let group_base_sum_coordinate_inspections = checked_parametric_mul(
                "polynomial-associate base-sum coordinate inspection bound",
                base_variables,
                cross_key_checks,
            )?;
            stats.base_sum_coordinate_inspection_bound = checked_parametric_add(
                "polynomial-associate base-sum coordinate inspection bound",
                stats.base_sum_coordinate_inspection_bound,
                group_base_sum_coordinate_inspections,
            )?;
            check_limit(
                "polynomial-associate base-sum coordinate inspection bound",
                stats.base_sum_coordinate_inspection_bound,
                limits.max_base_sum_coordinate_inspection_bound,
            )?;
            stats.base_exponent_addition_bound = checked_parametric_add(
                "polynomial-associate base-exponent addition bound",
                stats.base_exponent_addition_bound,
                checked_parametric_mul(
                    "polynomial-associate base-exponent addition bound",
                    group_base_sum_coordinate_inspections,
                    2,
                )?,
            )?;
            check_limit(
                "polynomial-associate base-exponent addition bound",
                stats.base_exponent_addition_bound,
                limits.max_base_exponent_addition_bound,
            )?;
        }
        if counted_cross_terms != stats.cross_terms {
            return Err(ParametricCoefficientError::Symbolica(
                "internal polynomial-associate anchor cost mismatch".to_owned(),
            ));
        }

        // Only after the full source-structural pass has been admitted do we
        // revisit the Cartesian pairs to census integer work.  Each
        // independently configurable integer/scratch limit is checked at the
        // update that first makes its new value visible, before the next pair
        // or derived-work calculation is entered.
        let mut visited_preflight_pairs = 0usize;
        for ordinal in 0..p_group_count {
            if ordinal == anchor {
                continue;
            }
            let p_group = p_groups[ordinal];
            let q_group = q_groups[ordinal];
            let cross_terms = associate_cross_term_count(p_group, q_group, p_anchor, q_anchor)?;
            let mut max_product_bits = 0usize;
            associate_visit_cross_pairs(
                &p_order,
                &q_order,
                p_group,
                q_group,
                p_anchor,
                q_anchor,
                |p_term, q_term, _| {
                    visited_preflight_pairs = checked_parametric_add(
                        "polynomial-associate integer preflight pairs",
                        visited_preflight_pairs,
                        1,
                    )?;
                    let p_bits = associate_integer_bit_count(&left.raw.coefficients[p_term])?;
                    let q_bits = associate_integer_bit_count(&right.raw.coefficients[q_term])?;
                    let p_limbs = associate_bits_to_units(
                        "polynomial-associate product scratch limbs",
                        p_bits,
                        64,
                    )?;
                    let q_limbs = associate_bits_to_units(
                        "polynomial-associate product scratch limbs",
                        q_bits,
                        64,
                    )?;
                    let product_bits = checked_parametric_add(
                        "polynomial-associate product bit bound",
                        p_bits,
                        q_bits,
                    )?;
                    max_product_bits = max_product_bits.max(product_bits);
                    let product_limbs = checked_parametric_add(
                        "polynomial-associate product scratch limbs",
                        p_limbs,
                        q_limbs,
                    )?;
                    stats.product_scratch_limbs = stats.product_scratch_limbs.max(product_limbs);
                    check_limit(
                        "polynomial-associate product scratch limbs",
                        stats.product_scratch_limbs,
                        limits.max_product_scratch_limbs,
                    )?;
                    let multiply_limb_operations = checked_parametric_mul(
                        "polynomial-associate integer multiply limb-operation bound",
                        p_limbs,
                        q_limbs,
                    )?;
                    stats.integer_multiply_limb_operation_bound = checked_parametric_add(
                        "polynomial-associate integer multiply limb-operation bound",
                        stats.integer_multiply_limb_operation_bound,
                        multiply_limb_operations,
                    )?;
                    check_limit(
                        "polynomial-associate integer multiply limb-operation bound",
                        stats.integer_multiply_limb_operation_bound,
                        limits.max_integer_multiply_limb_operation_bound,
                    )?;
                    stats.integer_multiplication_bit_work_bound = checked_parametric_add(
                        "polynomial-associate integer multiplication bit-work bound",
                        stats.integer_multiplication_bit_work_bound,
                        checked_parametric_mul(
                            "polynomial-associate integer multiplication bit-work bound",
                            p_bits,
                            q_bits,
                        )?,
                    )?;
                    check_limit(
                        "polynomial-associate integer multiplication bit-work bound",
                        stats.integer_multiplication_bit_work_bound,
                        limits.max_integer_multiplication_bit_work_bound,
                    )?;
                    stats.scratch_limb_write_bound = checked_parametric_add(
                        "polynomial-associate scratch limb-write bound",
                        stats.scratch_limb_write_bound,
                        associate_sum_counts(
                            "polynomial-associate scratch limb-write bound",
                            [product_limbs, multiply_limb_operations, p_limbs],
                        )?,
                    )?;
                    check_limit(
                        "polynomial-associate scratch limb-write bound",
                        stats.scratch_limb_write_bound,
                        limits.max_scratch_limb_write_bound,
                    )?;
                    Ok(())
                },
            )?;
            let accumulator_bits = checked_parametric_add(
                "polynomial-associate accumulator bit bound",
                max_product_bits,
                parametric_ceil_log2(cross_terms),
            )?;
            let accumulator_limbs = associate_bits_to_units(
                "polynomial-associate accumulator scratch limbs",
                accumulator_bits,
                64,
            )?;
            stats.accumulator_scratch_limbs =
                stats.accumulator_scratch_limbs.max(accumulator_limbs);
            check_limit(
                "polynomial-associate accumulator scratch limbs",
                stats.accumulator_scratch_limbs,
                limits.max_accumulator_scratch_limbs,
            )?;
            let accumulation_bit_work = checked_parametric_mul(
                "polynomial-associate integer accumulation bit-work bound",
                2,
                checked_parametric_mul(
                    "polynomial-associate integer accumulation bit-work bound",
                    cross_terms,
                    accumulator_bits,
                )?,
            )?;
            stats.integer_accumulation_bit_work_bound = checked_parametric_add(
                "polynomial-associate integer accumulation bit-work bound",
                stats.integer_accumulation_bit_work_bound,
                accumulation_bit_work,
            )?;
            check_limit(
                "polynomial-associate integer accumulation bit-work bound",
                stats.integer_accumulation_bit_work_bound,
                limits.max_integer_accumulation_bit_work_bound,
            )?;
            let accumulation_limb_bound = checked_parametric_mul(
                "polynomial-associate integer accumulation limb-operation bound",
                checked_parametric_mul(
                    "polynomial-associate integer accumulation limb-operation bound",
                    2,
                    cross_terms,
                )?,
                accumulator_limbs,
            )?;
            stats.integer_accumulate_limb_operation_bound = checked_parametric_add(
                "polynomial-associate integer accumulation limb-operation bound",
                stats.integer_accumulate_limb_operation_bound,
                accumulation_limb_bound,
            )?;
            check_limit(
                "polynomial-associate integer accumulation limb-operation bound",
                stats.integer_accumulate_limb_operation_bound,
                limits.max_integer_accumulate_limb_operation_bound,
            )?;
            stats.scratch_limb_write_bound = checked_parametric_add(
                "polynomial-associate scratch limb-write bound",
                stats.scratch_limb_write_bound,
                accumulation_limb_bound,
            )?;
            check_limit(
                "polynomial-associate scratch limb-write bound",
                stats.scratch_limb_write_bound,
                limits.max_scratch_limb_write_bound,
            )?;
        }
        if visited_preflight_pairs != stats.integer_preflight_pairs {
            return Err(ParametricCoefficientError::Symbolica(
                "internal polynomial-associate integer preflight census mismatch".to_owned(),
            ));
        }

        let span_envelope = associate_capacity_byte_envelope(
            stats.magnitude_copy_terms,
            size_of::<AssociateMagnitudeSpan>(),
        )?;
        let magnitude_envelope = associate_capacity_byte_envelope(stats.magnitude_copy_bytes, 1)?;
        let cross_envelope = associate_capacity_byte_envelope(
            stats.peak_cross_terms,
            size_of::<AssociateCrossRef>(),
        )?;
        let scratch_entries = checked_parametric_add(
            "polynomial-associate visible temporary byte envelope",
            stats.product_scratch_limbs,
            stats.accumulator_scratch_limbs,
        )?;
        stats.scratch_limb_write_bound = checked_parametric_add(
            "polynomial-associate scratch limb-write bound",
            stats.scratch_limb_write_bound,
            scratch_entries,
        )?;
        check_limit(
            "polynomial-associate scratch limb-write bound",
            stats.scratch_limb_write_bound,
            limits.max_scratch_limb_write_bound,
        )?;
        let scratch_envelope = associate_capacity_byte_envelope(scratch_entries, size_of::<u64>())?;
        stats.visible_temporary_byte_envelope = associate_sum_counts(
            "polynomial-associate visible temporary byte envelope",
            [
                fixed_headers,
                order_envelope,
                group_envelope,
                span_envelope,
                magnitude_envelope,
                cross_envelope,
                scratch_envelope,
            ],
        )?;
        check_associate_stats(&stats, limits)?;

        let mut spans = Vec::new();
        spans
            .try_reserve_exact(stats.magnitude_copy_terms)
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "allocation failed for polynomial-associate magnitude spans".to_owned(),
                )
            })?;
        let mut magnitude_arena = Vec::new();
        magnitude_arena
            .try_reserve_exact(stats.magnitude_copy_bytes)
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "allocation failed for polynomial-associate magnitude bytes".to_owned(),
                )
            })?;
        for coefficient in left.raw.coefficients.iter().chain(&right.raw.coefficients) {
            if spans.len() >= stats.magnitude_copy_terms {
                return Err(ParametricCoefficientError::Symbolica(
                    "internal polynomial-associate magnitude-span capacity exhausted".to_owned(),
                ));
            }
            spans.push(associate_append_magnitude(
                coefficient,
                &mut magnitude_arena,
            )?);
        }
        if spans.len() != stats.magnitude_copy_terms
            || magnitude_arena.len() != stats.magnitude_copy_bytes
        {
            return Err(ParametricCoefficientError::Symbolica(
                "internal polynomial-associate magnitude census mismatch".to_owned(),
            ));
        }
        associate_check_capacity(
            spans.capacity(),
            stats.magnitude_copy_terms,
            "polynomial-associate magnitude-span capacity",
        )?;
        associate_check_capacity(
            magnitude_arena.capacity(),
            stats.magnitude_copy_bytes,
            "polynomial-associate magnitude-byte capacity",
        )?;

        let mut cross_refs = Vec::new();
        cross_refs
            .try_reserve_exact(stats.peak_cross_terms)
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "allocation failed for polynomial-associate cross references".to_owned(),
                )
            })?;
        let mut product = Vec::new();
        product
            .try_reserve_exact(stats.product_scratch_limbs)
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "allocation failed for polynomial-associate product scratch".to_owned(),
                )
            })?;
        product.resize(stats.product_scratch_limbs, 0u64);
        let mut accumulator = Vec::new();
        accumulator
            .try_reserve_exact(stats.accumulator_scratch_limbs)
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "allocation failed for polynomial-associate accumulator scratch".to_owned(),
                )
            })?;
        accumulator.resize(stats.accumulator_scratch_limbs, 0u64);
        observed.scratch_limb_writes = checked_parametric_add(
            "polynomial-associate observed scratch limb writes",
            observed.scratch_limb_writes,
            scratch_entries,
        )?;
        associate_check_capacity(
            cross_refs.capacity(),
            stats.peak_cross_terms,
            "polynomial-associate cross-reference capacity",
        )?;
        associate_check_capacity(
            product.capacity(),
            stats.product_scratch_limbs,
            "polynomial-associate product-scratch capacity",
        )?;
        associate_check_capacity(
            accumulator.capacity(),
            stats.accumulator_scratch_limbs,
            "polynomial-associate accumulator-scratch capacity",
        )?;
        stats.visible_temporary_bytes = associate_sum_counts(
            "polynomial-associate visible temporary bytes",
            [
                fixed_headers,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    p_order.capacity(),
                    size_of::<usize>(),
                )?,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    q_order.capacity(),
                    size_of::<usize>(),
                )?,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    p_groups.capacity(),
                    size_of::<AssociateIndexGroup>(),
                )?,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    q_groups.capacity(),
                    size_of::<AssociateIndexGroup>(),
                )?,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    spans.capacity(),
                    size_of::<AssociateMagnitudeSpan>(),
                )?,
                magnitude_arena.capacity(),
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    cross_refs.capacity(),
                    size_of::<AssociateCrossRef>(),
                )?,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    product.capacity(),
                    size_of::<u64>(),
                )?,
                checked_parametric_mul(
                    "polynomial-associate visible temporary bytes",
                    accumulator.capacity(),
                    size_of::<u64>(),
                )?,
            ],
        )?;
        if stats.visible_temporary_bytes > stats.visible_temporary_byte_envelope {
            return Err(ParametricCoefficientError::ResourceLimit {
                resource: "polynomial-associate visible temporary bytes",
                requested: stats.visible_temporary_bytes,
                limit: stats.visible_temporary_byte_envelope,
            });
        }

        let mut associated = true;
        for ordinal in 0..p_group_count {
            if ordinal == anchor {
                continue;
            }
            let p_group = p_groups[ordinal];
            let q_group = q_groups[ordinal];
            let expected_cross_terms =
                associate_cross_term_count(p_group, q_group, p_anchor, q_anchor)?;
            cross_refs.clear();
            associate_visit_cross_pairs(
                &p_order,
                &q_order,
                p_group,
                q_group,
                p_anchor,
                q_anchor,
                |p_term, q_term, rhs| {
                    if cross_refs.len() >= stats.peak_cross_terms {
                        return Err(ParametricCoefficientError::Symbolica(
                            "internal polynomial-associate cross-reference capacity exhausted"
                                .to_owned(),
                        ));
                    }
                    cross_refs.push(AssociateCrossRef {
                        p_term,
                        q_term,
                        rhs,
                    });
                    Ok(())
                },
            )?;
            if cross_refs.len() != expected_cross_terms {
                return Err(ParametricCoefficientError::Symbolica(
                    "internal polynomial-associate cross-term census mismatch".to_owned(),
                ));
            }
            let group_order_bound = associate_heap_comparison_bound(cross_refs.len())?;
            observed.ordering_comparisons = checked_parametric_add(
                "polynomial-associate observed ordering comparisons",
                observed.ordering_comparisons,
                associate_heap_sort_by(
                    &mut cross_refs,
                    |left_ref, right_ref| {
                        associate_cross_ref_cmp(
                            &left.raw,
                            &right.raw,
                            left_ref,
                            right_ref,
                            base_variables,
                        )
                    },
                    group_order_bound,
                )?,
            )?;

            let mut accumulator_len = 0usize;
            let mut accumulator_negative = false;
            for cross_ordinal in 0..cross_refs.len() {
                if cross_ordinal > 0
                    && !associate_same_cross_key(
                        &left.raw,
                        &right.raw,
                        &cross_refs[cross_ordinal - 1],
                        &cross_refs[cross_ordinal],
                        base_variables,
                    )
                {
                    associated &= accumulator_len == 0;
                    accumulator_len = 0;
                    accumulator_negative = false;
                }
                let cross_ref = cross_refs[cross_ordinal];
                let p_span = spans[cross_ref.p_term];
                let q_span = spans[p_terms + cross_ref.q_term];
                let product_len = associate_multiply_magnitudes(
                    &magnitude_arena,
                    p_span,
                    q_span,
                    &mut product,
                    &mut observed,
                )?;
                let product_negative = left.raw.coefficients[cross_ref.p_term].is_negative()
                    ^ right.raw.coefficients[cross_ref.q_term].is_negative()
                    ^ cross_ref.rhs;
                associate_accumulate_product(
                    &mut accumulator,
                    &mut accumulator_len,
                    &mut accumulator_negative,
                    &product,
                    product_len,
                    product_negative,
                    &mut observed,
                )?;
            }
            associated &= accumulator_len == 0;
        }

        if observed.ordering_comparisons > stats.ordering_comparison_bound {
            return Err(ParametricCoefficientError::ResourceLimit {
                resource: "polynomial-associate observed ordering comparisons",
                requested: observed.ordering_comparisons,
                limit: stats.ordering_comparison_bound,
            });
        }
        if observed.integer_multiply_limb_operations != stats.integer_multiply_limb_operation_bound
        {
            return Err(ParametricCoefficientError::Symbolica(
                "internal polynomial-associate multiply work mismatch".to_owned(),
            ));
        }
        if observed.integer_accumulate_limb_operations
            > stats.integer_accumulate_limb_operation_bound
        {
            return Err(ParametricCoefficientError::ResourceLimit {
                resource: "polynomial-associate observed integer accumulation limb operations",
                requested: observed.integer_accumulate_limb_operations,
                limit: stats.integer_accumulate_limb_operation_bound,
            });
        }
        if observed.scratch_limb_writes > stats.scratch_limb_write_bound {
            return Err(ParametricCoefficientError::ResourceLimit {
                resource: "polynomial-associate observed scratch limb writes",
                requested: observed.scratch_limb_writes,
                limit: stats.scratch_limb_write_bound,
            });
        }
        Ok(ParametricPolynomialAssociateResult { associated, stats })
    }

    /// Prove exact divisibility in `K[n]`, where base-only polynomials are
    /// units of `K = Q(theta)`.
    ///
    /// This is intentionally one-way: `divisor | dividend` supports the
    /// integral-domain implications `divisor = 0 => dividend = 0` and
    /// `dividend != 0 => divisor != 0`.  It does not factor either input or
    /// infer radical-ideal membership.
    pub(crate) fn polynomial_divides_with_limits(
        &self,
        divisor: &ParametricPolynomial,
        dividend: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<bool, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(divisor, limits)?;
        self.validate_polynomial_with_limits(dividend, limits)?;
        if divisor.is_zero() || dividend.is_zero() {
            return Ok(false);
        }
        if divisor == dividend {
            return Ok(true);
        }
        let dividend_coefficient: Coefficient = try_copy_authenticated_sparse_polynomial_payload(
            &dividend.raw,
        )
        .map_err(|resource| {
            ParametricCoefficientError::Symbolica(format!(
                "allocation failed while copying {resource} for polynomial-divisibility proof"
            ))
        })?
        .into();
        let divisor_coefficient: Coefficient = try_copy_authenticated_sparse_polynomial_payload(
            &divisor.raw,
        )
        .map_err(|resource| {
            ParametricCoefficientError::Symbolica(format!(
                "allocation failed while copying {resource} for polynomial-divisibility proof"
            ))
        })?
        .into();
        let quotient = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            checked_coefficient_div_on_map(
                &dividend_coefficient,
                &divisor_coefficient,
                &self.variables,
                limits,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked during checked polynomial-divisibility proof".to_owned(),
            )
        })??;
        let first_index = self.base.variables().len();
        let denominator_uses_index = quotient.denominator.exponents_iter().any(|exponents| {
            exponents[first_index..]
                .iter()
                .any(|&exponent| exponent != 0)
        });
        Ok(!quotient.numerator.is_zero()
            && !quotient.denominator.is_zero()
            && !denominator_uses_index)
    }

    pub fn contains_base_polynomial(&self, value: &BasePolynomial) -> bool {
        value.context.as_ref() == self.base_fingerprint.as_ref()
            && validate_polynomial_on_map(
                &value.raw,
                self.base.variables(),
                crate::CoefficientPolynomialPart::Numerator,
                ExactAlgebraLimits::default(),
            )
            .is_ok()
    }

    pub fn zero(&self) -> ParametricCoefficient {
        self.wrap_unchecked(self.template.numerator.zero().into())
    }

    pub fn one(&self) -> ParametricCoefficient {
        self.wrap_unchecked(self.template.numerator.one().into())
    }

    pub fn integer(&self, value: i64) -> ParametricCoefficient {
        self.wrap_unchecked(
            self.template
                .numerator
                .constant(Integer::from(value))
                .into(),
        )
    }

    pub fn index(
        &self,
        position: usize,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        let variable = self.index_variables.get(position).ok_or(
            ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: position.saturating_add(1),
            },
        )?;
        let polynomial = self
            .template
            .numerator
            .variable(variable)
            .map_err(ParametricCoefficientError::Symbolica)?;
        Ok(self.wrap_unchecked(polynomial.into()))
    }

    pub fn lift(
        &self,
        value: &Coefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        if !self.base.contains(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        let numerator = self.extend_base_polynomial(&value.numerator)?;
        let denominator = self.extend_base_polynomial(&value.denominator)?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let raw = <Coefficient as FromNumeratorAndDenominator<
            IntegerRing,
            IntegerRing,
            u16,
        >>::from_num_den(numerator, denominator, &Z, true);
        self.wrap_checked(raw)
    }

    pub fn lift_base_polynomial(
        &self,
        value: &CoefficientPolynomial,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        let raw = self.extend_base_polynomial(value)?;
        Ok(ParametricPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    pub fn base_polynomial(
        &self,
        value: CoefficientPolynomial,
    ) -> Result<BasePolynomial, ParametricCoefficientError> {
        validate_polynomial_on_map(
            &value,
            self.base.variables(),
            crate::CoefficientPolynomialPart::Numerator,
            ExactAlgebraLimits::default(),
        )?;
        Ok(BasePolynomial {
            raw: value,
            context: self.base_fingerprint.clone(),
        })
    }

    pub fn numerator_condition(
        &self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.numerator_condition_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn numerator_condition_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_with_limits(value, limits)?;
        Ok(ParametricPolynomial {
            raw: value.raw.numerator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    pub fn denominator_condition(
        &self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.denominator_condition_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn denominator_condition_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_with_limits(value, limits)?;
        Ok(ParametricPolynomial {
            raw: value.raw.denominator.clone(),
            context: self.fingerprint.clone(),
        })
    }

    /// Multiply two authenticated index-polynomial conditions with separate
    /// checked Cartesian-work and retained-support preflights.
    ///
    /// This direct polynomial path intentionally avoids rational-function GCD
    /// normalization: exact quotients may densify a rational result, whereas
    /// a polynomial product has at most one retained term per point in its
    /// componentwise degree box.
    /// This is crate-private because callers must attach their own logical
    /// meaning to the resulting product locus.
    pub(crate) fn multiply_polynomial_conditions_with_limits(
        &self,
        left: &ParametricPolynomial,
        right: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.multiply_polynomial_conditions_with_limits_and_native_output_bound(
            left,
            right,
            limits,
            limits.max_polynomial_terms,
        )
    }

    /// Direct polynomial multiplication with an independent conservative
    /// native-output envelope.  `limits.max_polynomial_terms` continues to
    /// authenticate both inputs and the actual canonical output; the extra
    /// bound admits only the proved pre-native support envelope.
    pub(crate) fn multiply_polynomial_conditions_with_limits_and_native_output_bound(
        &self,
        left: &ParametricPolynomial,
        right: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
        max_native_output_term_bound: usize,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        if left.context.as_ref() != self.fingerprint.as_ref()
            || right.context.as_ref() != self.fingerprint.as_ref()
        {
            return Err(ParametricCoefficientError::WrongContext);
        }
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            checked_polynomial_mul_on_map(
                &left.raw,
                &right.raw,
                &self.variables,
                limits,
                max_native_output_term_bound,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked during checked polynomial multiplication".to_owned(),
            )
        })??;
        Ok(ParametricPolynomial {
            raw,
            context: self.fingerprint.clone(),
        })
    }

    pub fn add(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.add_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn add_with_limits(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_add_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn sub(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.sub_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn sub_with_limits(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_sub_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn mul(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.mul_with_limits(left, right, ExactAlgebraLimits::default())
    }

    pub fn mul_with_limits(
        &self,
        left: &ParametricCoefficient,
        right: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(left, limits)?;
        self.validate_with_limits(right, limits)?;
        let raw = checked_coefficient_mul_on_map(&left.raw, &right.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    pub fn neg(
        &self,
        value: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.neg_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn neg_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(value, limits)?;
        let raw = checked_coefficient_neg_on_map(&value.raw, &self.variables, limits)?;
        self.wrap_checked_with_limits(raw, limits)
    }

    /// Low-level exact field division.
    ///
    /// This intentionally returns no exceptional-domain provenance.  Rule
    /// discovery, pivot normalization, and other elimination-facing code must
    /// use [`Self::checked_div_guarded`] instead.
    pub fn checked_div(
        &self,
        numerator: &ParametricCoefficient,
        denominator: &ParametricCoefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.checked_div_with_limits(numerator, denominator, ExactAlgebraLimits::default())
    }

    pub fn checked_div_with_limits(
        &self,
        numerator: &ParametricCoefficient,
        denominator: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(numerator, limits)?;
        self.validate_with_limits(denominator, limits)?;
        let raw = checked_coefficient_div_on_map(
            &numerator.raw,
            &denominator.raw,
            &self.variables,
            limits,
        )
        .map_err(|error| match error {
            ExactAlgebraError::DivisionByZero => ParametricCoefficientError::DivisionByZero,
            other => ParametricCoefficientError::ExactAlgebra(other),
        })?;
        self.wrap_checked_with_limits(raw, limits)
    }

    /// Divide while retaining every pre-cancellation condition needed by the
    /// two rational operands.
    ///
    /// For `A/B` divided by `C/D`, the returned domain contains `B != 0`,
    /// `D != 0`, and `C != 0` (nonzero constants are omitted).  In
    /// particular, `0 / n` still returns the mandatory `n != 0` condition
    /// even though its normalized value is zero.
    pub fn checked_div_guarded(
        &self,
        dividend: &ParametricCoefficient,
        divisor: &ParametricCoefficient,
    ) -> Result<GuardedParametricCoefficient, ParametricCoefficientError> {
        self.checked_div_guarded_with_limits(dividend, divisor, ExactAlgebraLimits::default())
    }

    pub fn checked_div_guarded_with_limits(
        &self,
        dividend: &ParametricCoefficient,
        divisor: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<GuardedParametricCoefficient, ParametricCoefficientError> {
        let pending =
            self.checked_div_guarded_pending_normalization_with_limits(dividend, divisor, limits)?;
        self.finish_guarded_division_normalization_with_limits(pending, limits)
    }

    /// Perform guarded division through Symbolica's first checked quotient,
    /// stopping immediately before RustRed's explicit second normalization.
    ///
    pub(crate) fn checked_div_guarded_pending_normalization_with_limits(
        &self,
        dividend: &ParametricCoefficient,
        divisor: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<PendingGuardedParametricDivision, ParametricCoefficientError> {
        self.checked_div_guarded_pending_normalization_with_limits_and_origin_limit(
            dividend,
            divisor,
            limits,
            ParametricArithmeticLimits::default().max_guard_origins,
        )
    }

    /// Guarded-division pending phase with an explicit provenance-cardinality
    /// ceiling. Database work ledgers use this seam so construction and replay
    /// cannot silently fall back to the public default origin budget.
    pub(crate) fn checked_div_guarded_pending_normalization_with_limits_and_origin_limit(
        &self,
        dividend: &ParametricCoefficient,
        divisor: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
        max_guard_origins: usize,
    ) -> Result<PendingGuardedParametricDivision, ParametricCoefficientError> {
        self.validate_with_limits(dividend, limits)?;
        self.validate_with_limits(divisor, limits)?;
        if divisor.raw.numerator.is_zero() {
            return Err(ParametricCoefficientError::DivisionByZero);
        }

        // Clone all three source polynomials before Symbolica normalizes the
        // quotient.  Equal conditions merge their origin sets below.
        let candidates = [
            (
                dividend.raw.denominator.clone(),
                GuardOrigin::GuardedDivisionDividendDenominator,
            ),
            (
                divisor.raw.denominator.clone(),
                GuardOrigin::GuardedDivisionDivisorDenominator,
            ),
            (
                divisor.raw.numerator.clone(),
                GuardOrigin::GuardedDivisionDivisorNumerator,
            ),
        ];
        let mut nonzero = Vec::with_capacity(candidates.len());
        for (raw, origin) in candidates {
            if raw.is_constant() {
                debug_assert!(!raw.is_zero());
                continue;
            }
            let polynomial = ParametricPolynomial {
                raw,
                context: self.fingerprint.clone(),
            };
            self.validate_polynomial_with_limits(&polynomial, limits)?;
            let condition = self.nonzero_condition_with_origins_and_origin_limit(
                polynomial,
                [origin],
                limits,
                max_guard_origins,
            )?;
            insert_parametric_condition(&mut nonzero, condition, max_guard_origins)?;
        }

        let value = self.checked_div_with_limits(dividend, divisor, limits)?;
        Ok(PendingGuardedParametricDivision { value, nonzero })
    }

    /// Construct an authenticated but deliberately noncanonical pending
    /// fraction for testing the explicit second guarded-division
    /// normalization. Production constructors continue to preserve the
    /// canonical [`ParametricCoefficient`] invariant.
    #[cfg(test)]
    pub(crate) fn noncanonical_pending_fraction_for_test(
        &self,
        numerator: &ParametricPolynomial,
        denominator: &ParametricPolynomial,
        limits: ExactAlgebraLimits,
    ) -> Result<PendingGuardedParametricDivision, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(numerator, limits)?;
        self.validate_polynomial_with_limits(denominator, limits)?;
        if denominator.raw.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let value = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: numerator.raw.clone(),
                denominator: denominator.raw.clone(),
            },
            context: self.fingerprint.clone(),
        };
        self.validate_with_limits(&value, limits)?;
        Ok(PendingGuardedParametricDivision {
            value,
            nonzero: Vec::new(),
        })
    }

    /// Fabricate a provenance-bearing zero condition for source-admission
    /// precedence tests. Production condition constructors reject this value.
    #[cfg(test)]
    pub(crate) fn zero_nonzero_condition_for_test(&self) -> ParametricNonZeroCondition {
        ParametricNonZeroCondition {
            polynomial: ParametricPolynomial {
                raw: self.template.numerator.zero(),
                context: self.fingerprint.clone(),
            },
            origins: BTreeSet::from([GuardOrigin::ExplicitRelationCondition]),
        }
    }

    /// Finish a pending guarded division after its actual normalization input
    /// has passed the caller's prospective work census.
    pub(crate) fn finish_guarded_division_normalization_with_limits(
        &self,
        pending: PendingGuardedParametricDivision,
        limits: ExactAlgebraLimits,
    ) -> Result<GuardedParametricCoefficient, ParametricCoefficientError> {
        self.finish_guarded_division_normalization_with_limits_and_origin_limit(
            pending,
            limits,
            ParametricArithmeticLimits::default().max_guard_origins,
        )
    }

    /// Finish a pending guarded division under the caller's provenance
    /// ceiling as well as its exact-algebra limits.
    ///
    /// A pending value is crate-private but may have been produced by a
    /// different work facade or by the compatibility seam above. Recheck
    /// every retained condition before normalization so a stricter ledger
    /// cannot accept a pending value constructed under the default origin
    /// budget. `max_guard_origins` is, consistently with condition insertion,
    /// a per-condition ceiling; aggregate guard-vector cardinality belongs to
    /// the enclosing relation or certificate budget.
    pub(crate) fn finish_guarded_division_normalization_with_limits_and_origin_limit(
        &self,
        pending: PendingGuardedParametricDivision,
        limits: ExactAlgebraLimits,
        max_guard_origins: usize,
    ) -> Result<GuardedParametricCoefficient, ParametricCoefficientError> {
        for condition in &pending.nonzero {
            if condition.origins.is_empty() {
                return Err(ParametricCoefficientError::MissingGuardOrigin);
            }
            check_limit(
                "parametric guard origins",
                condition.origins.len(),
                max_guard_origins,
            )?;
            self.validate_polynomial_with_limits(&condition.polynomial, limits)?;
        }
        let value = self.normalize_with_limits(pending.value, limits)?;
        Ok(GuardedParametricCoefficient {
            value,
            nonzero: pending.nonzero,
        })
    }

    /// Allocation-free preflight for one authenticated polynomial translation.
    pub(crate) fn preflight_translate_polynomial(
        &self,
        value: &ParametricPolynomial,
        shift: &IndexShift,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialTranslationPreflight, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(shift.values())?;
        self.preflight_translate_polynomial_raw(&value.raw, shift.values(), limits)
    }

    /// Allocation-free preflight for an arbitrary-precision integer
    /// translation.  Raw Symbolica integer representations are accepted at
    /// this borrowed boundary, but execution canonicalizes every component
    /// before it can enter a polynomial.
    pub(crate) fn preflight_translate_polynomial_exact(
        &self,
        value: &ParametricPolynomial,
        shift: &[Integer],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialTranslationPreflight, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_exact_shift(shift)?;
        self.preflight_translate_polynomial_raw(&value.raw, shift, limits)
    }

    /// Allocation-free preflight for both halves and the normalization of one
    /// authenticated rational translation.
    pub(crate) fn preflight_translate_coefficient(
        &self,
        value: &ParametricCoefficient,
        shift: &IndexShift,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficientTranslationPreflight, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(shift.values())?;
        let numerator =
            self.preflight_translate_polynomial_raw(&value.raw.numerator, shift.values(), limits)?;
        let denominator = self.preflight_translate_polynomial_raw(
            &value.raw.denominator,
            shift.values(),
            limits,
        )?;
        coefficient_translation_preflight(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator,
            denominator,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.variables.len(),
            limits,
        )
    }

    /// Allocation-free preflight for both halves and normalization of one
    /// arbitrary-precision integer translation.
    pub(crate) fn preflight_translate_coefficient_exact(
        &self,
        value: &ParametricCoefficient,
        shift: &[Integer],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficientTranslationPreflight, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_exact_shift(shift)?;
        let numerator =
            self.preflight_translate_polynomial_raw(&value.raw.numerator, shift, limits)?;
        let denominator =
            self.preflight_translate_polynomial_raw(&value.raw.denominator, shift, limits)?;
        coefficient_translation_preflight(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator,
            denominator,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.variables.len(),
            limits,
        )
    }

    /// Apply `n -> n + shift` to a complete coefficient.
    pub fn translate(
        &self,
        value: &ParametricCoefficient,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(shift)?;
        self.translate_coefficient_validated(value, shift, limits)
    }

    /// Apply `n -> n + shift` using canonical arbitrary-precision Symbolica
    /// integers.  This is crate-private until the generated group database can
    /// bind the exact recentering event and its guard-provenance locator.
    pub(crate) fn translate_exact(
        &self,
        value: &ParametricCoefficient,
        shift: &[Integer],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_exact_shift(shift)?;
        self.translate_coefficient_validated(value, shift, limits)
    }

    fn translate_coefficient_validated<T: ParametricTranslationComponent>(
        &self,
        value: &ParametricCoefficient,
        shift: &[T],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        let numerator_preflight =
            self.preflight_translate_polynomial_raw(&value.raw.numerator, shift, limits)?;
        let denominator_preflight =
            self.preflight_translate_polynomial_raw(&value.raw.denominator, shift, limits)?;
        let preflight = coefficient_translation_preflight(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator_preflight,
            denominator_preflight,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.variables.len(),
            limits,
        )?;
        let numerator = self.execute_translate_polynomial_raw(
            &value.raw.numerator,
            shift,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_translate_polynomial_raw(
            &value.raw.denominator,
            shift,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Translation is a polynomial-ring automorphism. The validated
            // source numerator and denominator are coprime, hence their
            // translated images are coprime too. Avoid a redundant native
            // GCD and its otherwise unbounded transient workspace.
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                false,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while normalizing a checked parametric translation".to_owned(),
            )
        })?;
        let wrapped = self.wrap_checked_with_limits(raw, limits.exact_algebra)?;
        verify_translated_coefficient_envelope(&wrapped, preflight)?;
        Ok(wrapped)
    }

    pub fn translate_polynomial(
        &self,
        value: &ParametricPolynomial,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(shift)?;
        Ok(ParametricPolynomial {
            raw: self.translate_polynomial_raw(&value.raw, shift, limits)?,
            context: self.fingerprint.clone(),
        })
    }

    pub(crate) fn translate_polynomial_exact(
        &self,
        value: &ParametricPolynomial,
        shift: &[Integer],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_exact_shift(shift)?;
        Ok(ParametricPolynomial {
            raw: self.translate_polynomial_raw(&value.raw, shift, limits)?,
            context: self.fingerprint.clone(),
        })
    }

    /// Translate a guard polynomial while preserving its source origins and
    /// recording the affine index map that changed its locus.
    pub fn translate_nonzero_condition(
        &self,
        value: &ParametricNonZeroCondition,
        shift: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        if !self.contains_nonzero_condition(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        self.validate_shift(shift)?;
        let already_has_translation = value.origins.iter().any(|origin| {
            matches!(
                origin,
                GuardOrigin::IndexTranslation { offset } if offset.as_ref() == shift
            )
        });
        let final_origin_count = value
            .origins
            .len()
            .checked_add(usize::from(!already_has_translation))
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric guard origins",
            })?;
        check_limit(
            "parametric guard origins",
            final_origin_count,
            limits.max_guard_origins,
        )?;
        let polynomial = self.translate_polynomial(&value.polynomial, shift, limits)?;
        let mut origins = value.origins.clone();
        if !already_has_translation {
            let mut offset = Vec::new();
            offset.try_reserve_exact(shift.len()).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "index translation origin components",
                }
            })?;
            offset.extend_from_slice(shift);
            origins.insert(GuardOrigin::IndexTranslation {
                offset: offset.into_boxed_slice(),
            });
        }
        self.nonzero_condition_with_origins_and_limits(polynomial, origins, limits.exact_algebra)
    }

    /// Simultaneously rename the private index variables by a bijection.
    ///
    /// `source_to_target[i] = j` means
    /// `n_source[i] -> n_target[j]`.  This is the coefficient half of
    /// transporting a complete parametric identity through a proved
    /// denominator permutation.  It must not be used to canonicalize one
    /// isolated `I(n+s)` term: for generic `n`, a nontrivial permutation sends
    /// `n` to `P n`, not back to `n`.
    pub fn permute_indices(
        &self,
        value: &ParametricCoefficient,
        source_to_target: &[usize],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_index_permutation(source_to_target)?;
        let numerator =
            self.permute_polynomial_raw(&value.raw.numerator, source_to_target, limits)?;
        let denominator =
            self.permute_polynomial_raw(&value.raw.denominator, source_to_target, limits)?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let raw = <Coefficient as FromNumeratorAndDenominator<
            IntegerRing,
            IntegerRing,
            u16,
        >>::from_num_den(numerator, denominator, &Z, true);
        self.wrap_checked_with_limits(raw, limits.exact_algebra)
    }

    pub fn permute_polynomial_indices(
        &self,
        value: &ParametricPolynomial,
        source_to_target: &[usize],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_index_permutation(source_to_target)?;
        Ok(ParametricPolynomial {
            raw: self.permute_polynomial_raw(&value.raw, source_to_target, limits)?,
            context: self.fingerprint.clone(),
        })
    }

    pub fn permute_nonzero_condition_indices(
        &self,
        value: &ParametricNonZeroCondition,
        source_to_target: &[usize],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        if !self.contains_nonzero_condition(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        self.validate_index_permutation(source_to_target)?;
        let polynomial =
            self.permute_polynomial_indices(value.polynomial(), source_to_target, limits)?;
        let mut origins = value.origins.clone();
        origins.insert(GuardOrigin::IndexPermutation {
            source_to_target: source_to_target.to_vec().into_boxed_slice(),
        });
        check_limit(
            "parametric guard origins",
            origins.len(),
            limits.max_guard_origins,
        )?;
        self.nonzero_condition_with_origins_and_limits(polynomial, origins, limits.exact_algebra)
    }

    /// Allocation-free preflight for projecting one authenticated polynomial
    /// from `K(n)` to `K` at a complete integer assignment.
    pub(crate) fn preflight_specialize_polynomial(
        &self,
        value: &ParametricPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialSpecializationPreflight, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(assignment)?;
        self.preflight_specialize_polynomial_raw(&value.raw, assignment, limits)
    }

    /// Allocation-free preflight for both mapped halves, retained denominator
    /// guard, and normalized coefficient at a complete integer assignment.
    pub(crate) fn preflight_specialize_coefficient(
        &self,
        value: &ParametricCoefficient,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficientSpecializationPreflight, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(assignment)?;
        let numerator =
            self.preflight_specialize_polynomial_raw(&value.raw.numerator, assignment, limits)?;
        let denominator =
            self.preflight_specialize_polynomial_raw(&value.raw.denominator, assignment, limits)?;
        coefficient_specialization_preflight(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator,
            denominator,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.base.variables().len(),
            limits,
        )
    }

    /// Simultaneously specialize every index and project the result to the
    /// exact base variable map.  The original mapped denominator is retained
    /// as a nonzero condition even when normalization cancels it.
    pub fn specialize(
        &self,
        value: &ParametricCoefficient,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<GuardedCoefficientSpecialization, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_shift(assignment)?;
        let numerator_preflight =
            self.preflight_specialize_polynomial_raw(&value.raw.numerator, assignment, limits)?;
        let denominator_preflight =
            self.preflight_specialize_polynomial_raw(&value.raw.denominator, assignment, limits)?;
        let preflight = coefficient_specialization_preflight(
            &value.raw.numerator,
            &value.raw.denominator,
            numerator_preflight,
            denominator_preflight,
            value.raw.numerator.is_zero(),
            value.raw.denominator.is_one(),
            self.base.variables().len(),
            limits,
        )?;
        let numerator = self.execute_specialize_polynomial_raw(
            &value.raw.numerator,
            assignment,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_specialize_polynomial_raw(
            &value.raw.denominator,
            assignment,
            limits,
            denominator_preflight,
        )?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }
        let mut nonzero = Vec::new();
        let mut guarded_nonzero = Vec::new();
        if !denominator.is_constant() {
            let polynomial = BasePolynomial {
                raw: denominator.clone(),
                context: self.base_fingerprint.clone(),
            };
            let origins = BTreeSet::from([
                GuardOrigin::CoefficientSpecializationDenominator,
                GuardOrigin::IndexSpecialization {
                    assignment: assignment.to_vec().into_boxed_slice(),
                },
            ]);
            check_limit(
                "specialized guard origins",
                origins.len(),
                limits.max_guard_origins,
            )?;
            nonzero.push(polynomial.clone());
            guarded_nonzero.push(SpecializedNonZeroCondition {
                polynomial,
                origins,
            });
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                true,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while normalizing a checked coefficient specialization"
                    .to_owned(),
            )
        })?;
        if !self.base.contains(&result) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        verify_specialized_coefficient_envelope(&result, &guarded_nonzero, preflight)?;
        Ok(GuardedCoefficientSpecialization {
            value: result,
            nonzero,
            guarded_nonzero,
        })
    }

    pub fn specialize_polynomial(
        &self,
        value: &ParametricPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<BasePolynomial, ParametricCoefficientError> {
        self.validate_parametric_polynomial(value)?;
        self.validate_shift(assignment)?;
        Ok(BasePolynomial {
            raw: self.specialize_polynomial_raw(&value.raw, assignment, limits)?,
            context: self.base_fingerprint.clone(),
        })
    }

    /// Substitute a canonical sparse subset of index variables while keeping
    /// all remaining variables on this exact authenticated `K(n)` map.
    pub fn partially_specialize_polynomial(
        &self,
        value: &ParametricPolynomial,
        assignment: &PartialIndexAssignment,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        Ok(self
            .partially_specialize_polynomial_checked(value, assignment, limits)?
            .0)
    }

    /// Partially specialize a rational coefficient and retain the mapped
    /// original denominator as a provenance-bearing nonzero condition before
    /// normalization can cancel it.
    pub fn partially_specialize_coefficient(
        &self,
        value: &ParametricCoefficient,
        assignment: &PartialIndexAssignment,
        limits: ParametricArithmeticLimits,
    ) -> Result<GuardedPartialCoefficientSpecialization, ParametricCoefficientError> {
        self.validate_with_limits(value, limits.exact_algebra)?;
        self.validate_partial_assignment(assignment)?;
        let (numerator, numerator_stats) = self.partially_specialize_polynomial_raw_checked(
            &value.raw.numerator,
            assignment,
            limits,
        )?;
        let (denominator, denominator_stats) = self.partially_specialize_polynomial_raw_checked(
            &value.raw.denominator,
            assignment,
            limits,
        )?;
        if denominator.is_zero() {
            return Err(ParametricCoefficientError::ZeroDenominator);
        }

        let mut nonzero = Vec::new();
        let mut guarded_nonzero = Vec::new();
        if !denominator.is_constant() {
            let polynomial = ParametricPolynomial {
                raw: denominator.clone(),
                context: self.fingerprint.clone(),
            };
            let condition = self.nonzero_condition_with_origins_and_origin_limit(
                polynomial.clone(),
                [
                    GuardOrigin::CoefficientPartialSpecializationDenominator,
                    assignment.provenance_origin(),
                ],
                limits.exact_algebra,
                limits.max_guard_origins,
            )?;
            nonzero.push(polynomial);
            guarded_nonzero.push(condition);
        }

        let normalization_operations = numerator.nterms().checked_mul(denominator.nterms()).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "partial coefficient normalization term pairs",
            },
        )?;
        check_limit(
            "partial coefficient normalization term pairs",
            normalization_operations,
            limits.exact_algebra.max_term_operations,
        )?;
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<
                IntegerRing,
                IntegerRing,
                u16,
            >>::from_num_den(numerator, denominator, &Z, true)
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while normalizing a checked partial coefficient specialization"
                    .to_owned(),
            )
        })?;
        let value = self.wrap_checked_with_limits(raw, limits.exact_algebra)?;
        let stats = PartialPolynomialSpecializationStats {
            source_terms: checked_partial_stat_add(
                "partial coefficient specialization source terms",
                numerator_stats.source_terms,
                denominator_stats.source_terms,
            )?,
            output_terms: checked_partial_stat_add(
                "partial coefficient specialization output terms",
                numerator_stats.output_terms,
                denominator_stats.output_terms,
            )?,
            power_operations: checked_partial_stat_add(
                "partial coefficient specialization power operations",
                numerator_stats.power_operations,
                denominator_stats.power_operations,
            )?,
            integer_bit_bound: checked_partial_stat_add(
                "partial coefficient specialization integer bits",
                numerator_stats.integer_bit_bound,
                denominator_stats.integer_bit_bound,
            )?,
        };
        Ok(GuardedPartialCoefficientSpecialization {
            value,
            nonzero,
            assignment: assignment.clone(),
            guarded_nonzero,
            stats,
        })
    }

    /// Specialize one existing guard on a sparse equality locus, preserving
    /// every prior origin and adding the canonical assignment transcript.
    pub fn partially_specialize_nonzero_condition(
        &self,
        value: &ParametricNonZeroCondition,
        assignment: &PartialIndexAssignment,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricNonZeroCondition, ParametricCoefficientError> {
        if !self.contains_nonzero_condition(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        self.validate_partial_assignment(assignment)?;
        let polynomial =
            self.partially_specialize_polynomial(value.polynomial(), assignment, limits)?;
        let mut origins = value.origins.clone();
        origins.insert(assignment.provenance_origin());
        check_limit(
            "parametric guard origins",
            origins.len(),
            limits.max_guard_origins,
        )?;
        self.nonzero_condition_with_origins_and_origin_limit(
            polynomial,
            origins,
            limits.exact_algebra,
            limits.max_guard_origins,
        )
    }

    pub(crate) fn partially_specialize_polynomial_checked(
        &self,
        value: &ParametricPolynomial,
        assignment: &PartialIndexAssignment,
        limits: ParametricArithmeticLimits,
    ) -> Result<
        (ParametricPolynomial, PartialPolynomialSpecializationStats),
        ParametricCoefficientError,
    > {
        self.validate_polynomial_with_limits(value, limits.exact_algebra)?;
        self.validate_partial_assignment(assignment)?;
        let (raw, stats) =
            self.partially_specialize_polynomial_raw_checked(&value.raw, assignment, limits)?;
        Ok((
            ParametricPolynomial {
                raw,
                context: self.fingerprint.clone(),
            },
            stats,
        ))
    }

    /// Substitute one denominator-index variable by an exact integer while
    /// preserving the complete authenticated `K(n)` variable map.
    ///
    /// This is the bounded partial-specialization primitive used by symbolic
    /// sector-boundary proofs.  Unlike [`Self::specialize_polynomial`], the
    /// other index variables remain symbolic and the result is therefore a
    /// [`ParametricPolynomial`].
    pub fn specialize_polynomial_index(
        &self,
        source: &ParametricPolynomial,
        position: usize,
        value: i64,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomial, ParametricCoefficientError> {
        self.validate_polynomial_with_limits(source, limits.exact_algebra)?;
        if position >= self.index_count() {
            return Err(ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: position.saturating_add(1),
            });
        }
        check_limit(
            "partial polynomial specialization source terms",
            source.raw.nterms(),
            limits.max_source_terms,
        )?;
        check_limit(
            "partial polynomial specialization power operations",
            source.raw.nterms(),
            limits.max_specialization_power_operations,
        )?;
        // Substitution cannot produce more sparse terms than it consumes, but
        // Symbolica allocates the result before we can inspect it. Preflight
        // that conservative output bound before entering the library call.
        check_limit(
            "partial polynomial specialization output terms",
            source.raw.nterms(),
            limits.max_output_terms,
        )?;

        let variable = self.base.variables().len() + position;
        let magnitude = value.unsigned_abs();
        let value_bits = u128::from(u64::BITS - magnitude.leading_zeros());
        let mut largest_term_bits = 0usize;
        for (coefficient, exponents) in source
            .raw
            .coefficients
            .iter()
            .zip(source.raw.exponents_iter())
        {
            let mut requested = integer_magnitude_bits(coefficient);
            let exponent = exponents[variable];
            if magnitude > 1 && exponent != 0 {
                requested = requested
                    .checked_add(value_bits.checked_mul(u128::from(exponent)).ok_or(
                        ParametricCoefficientError::ResourceCountOverflow {
                            resource: "partial polynomial specialization integer bits",
                        },
                    )?)
                    .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                        resource: "partial polynomial specialization integer bits",
                    })?;
            }
            let requested = usize::try_from(requested).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "partial polynomial specialization integer bits",
                }
            })?;
            check_limit(
                "partial polynomial specialization integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_term_bits = largest_term_bits.max(requested);
        }

        // Removing one exponent can merge up to all source monomials. The
        // magnitude of a sum of N integers with at most B bits is bounded by
        // B + ceil(log2(N)); preflight that collected coefficient too.
        let collision_bits = if source.raw.nterms() <= 1 {
            0
        } else {
            usize::BITS as usize - (source.raw.nterms() - 1).leading_zeros() as usize
        };
        let collected_bits = largest_term_bits.checked_add(collision_bits).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "partial polynomial specialization integer bits",
            },
        )?;
        check_limit(
            "partial polynomial specialization integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;

        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            source.raw.replace(variable, &Integer::from(value))
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked during checked partial polynomial specialization".to_owned(),
            )
        })?;
        check_limit(
            "partial polynomial specialization output terms",
            raw.nterms(),
            limits.max_output_terms,
        )?;
        let result = ParametricPolynomial {
            raw,
            context: self.fingerprint.clone(),
        };
        self.validate_polynomial_with_limits(&result, limits.exact_algebra)?;
        Ok(result)
    }

    /// Specialize one existing parametric condition and retain all source
    /// provenance alongside the exact assignment.
    pub fn specialize_nonzero_condition(
        &self,
        value: &ParametricNonZeroCondition,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<SpecializedNonZeroCondition, ParametricCoefficientError> {
        if !self.contains_nonzero_condition(value) {
            return Err(ParametricCoefficientError::WrongContext);
        }
        self.validate_shift(assignment)?;
        let polynomial = self.specialize_polynomial(&value.polynomial, assignment, limits)?;
        let mut origins = value.origins.clone();
        origins.insert(GuardOrigin::IndexSpecialization {
            assignment: assignment.to_vec().into_boxed_slice(),
        });
        check_limit(
            "specialized guard origins",
            origins.len(),
            limits.max_guard_origins,
        )?;
        Ok(SpecializedNonZeroCondition {
            polynomial,
            origins,
        })
    }

    /// Compile the one immutable full-point image used by every polynomial
    /// restricted to the same certified residual unit-affine locus.
    ///
    /// This remains crate-private deliberately: an affine map gives a sound
    /// meaning to shifted integrals only while it is attached to the future
    /// affine-locus relation wrapper.  Exposing the raw compositor would make
    /// it too easy to relabel the resulting row as a global identity.
    pub(crate) fn compile_residual_unit_affine_composition_plan(
        &self,
        map: Arc<ResidualUnitAffineIndexMapCertificate>,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<ResidualUnitAffineCompositionPlan, ResidualUnitAffineCompositionError> {
        if map.context_fingerprint() != self.fingerprint() {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        if map.ambient_arity() != self.index_count() {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: self.index_count(),
                actual: map.ambient_arity(),
            });
        }
        map.replay(self)?;
        let geometry = compact_legacy_unit_affine_geometry(
            &map,
            self.index_count(),
            self.base.variables().len(),
            limits,
        )?;
        let core = Arc::new(self.compile_residual_affine_composition_core(geometry, limits)?);
        Ok(ResidualUnitAffineCompositionPlan {
            schema: RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA,
            context_fingerprint: self.fingerprint.clone(),
            map,
            core,
            limits,
        })
    }

    /// Compile an authority-neutral V2 plan from exact compact affine
    /// geometry.
    ///
    /// The complete prospective term/exponent/GMP-bit/logical-byte census is
    /// checked before `materialize_residual_affine_compact_geometry` performs
    /// the first user-sized allocation.  No V1 certificate is fabricated or
    /// retained.
    pub(crate) fn compile_residual_affine_compact_composition_plan(
        &self,
        geometry: ResidualAffineCompactMapView<'_>,
        limits: ResidualAffineCompactCompositionPlanLimits,
    ) -> Result<ResidualAffineCompactCompositionPlan, ResidualUnitAffineCompositionError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.compile_residual_affine_compact_composition_plan_unwind_boundary(geometry, limits)
        }))
        .map_err(|_| ResidualUnitAffineCompositionError::SymbolicaPanic {
            stage: "compact affine composition plan compilation",
        })?
    }

    fn compile_residual_affine_compact_composition_plan_unwind_boundary(
        &self,
        geometry: ResidualAffineCompactMapView<'_>,
        limits: ResidualAffineCompactCompositionPlanLimits,
    ) -> Result<ResidualAffineCompactCompositionPlan, ResidualUnitAffineCompositionError> {
        #[cfg(test)]
        maybe_inject_residual_affine_compact_boundary_panic_for_test();
        let preflight = residual_affine_compact_geometry_preflight(self, geometry, limits)?;
        let compact = materialize_residual_affine_compact_geometry(geometry, preflight)?;
        let core =
            Arc::new(self.compile_residual_affine_composition_core(compact, limits.composition)?);
        if core.stats != preflight.composition {
            return Err(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "compact affine composition prospective statistics",
                    actual: core.stats.total_image_terms,
                    bound: preflight.composition.total_image_terms,
                },
            );
        }
        replay_residual_affine_compact_geometry_against_core(self, &core, geometry, preflight)?;
        let stats = ResidualAffineCompactCompositionPlanStats {
            composition: preflight.composition,
            context_fingerprint_bytes: self.fingerprint().len(),
            geometry_integer_entries_inspected: preflight.geometry_integer_entries_inspected,
            geometry_integer_bit_work: preflight.geometry_integer_bit_work,
            geometry_replay_comparison_work: preflight.geometry_replay_comparison_work,
            geometry_replay_integer_bit_work: preflight.geometry_replay_integer_bit_work,
            geometry_replay_scratch_logical_bytes: preflight.geometry_replay_scratch_logical_bytes,
            retained_owned_logical_bytes: preflight.retained_owned_logical_bytes,
            compilation_owned_logical_peak_upper_bound: preflight
                .compilation_owned_logical_peak_upper_bound,
        };
        let manifest = ResidualAffineCompactCompositionManifest {
            schema: RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA,
            context_fingerprint_bytes: self.fingerprint().len(),
            context_checksum: residual_affine_diagnostic_checksum(self.fingerprint().as_bytes()),
            ambient_arity: geometry.ambient_arity,
            free_count: geometry.free_positions.len(),
            geometry_checksum: preflight.geometry_checksum,
            limits,
            stats,
        };
        let plan = ResidualAffineCompactCompositionPlan {
            schema: RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA,
            context_fingerprint: self.fingerprint.clone(),
            geometry_checksum: preflight.geometry_checksum,
            core,
            limits,
            stats,
            manifest,
        };
        self.validate_residual_affine_compact_composition_plan(&plan)?;
        Ok(plan)
    }

    /// Compile simultaneous polynomial images from a replay-authenticated
    /// ambient-square integer-system affine map.
    pub(crate) fn compile_residual_affine_composition_plan_from_integer_system(
        &self,
        certificate: Arc<ResidualAffineIntegerSystemCertificate>,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<ResidualAffineCompositionPlan, ResidualUnitAffineCompositionError> {
        if certificate.ambient_arity() != self.index_count() {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: self.index_count(),
                actual: certificate.ambient_arity(),
            });
        }
        certificate.replay()?;
        self.compile_residual_affine_composition_plan_from_authenticated_integer_system(
            certificate,
            limits,
        )
    }

    /// V2-only plan construction from the unforgeable result of the immediately
    /// preceding integer-system compilation. Every adjacent raw census is
    /// reauthenticated, while only the redundant V1 integer-system replay is
    /// omitted.
    pub(crate) fn compile_residual_affine_composition_plan_from_fresh_integer_system(
        &self,
        authorization: ResidualAffineIntegerSystemFreshPlanAuthorization,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<ResidualAffineCompositionPlan, ResidualUnitAffineCompositionError> {
        let certificate = authorization.into_authenticated_certificate_arc()?;
        self.compile_residual_affine_composition_plan_from_authenticated_integer_system(
            certificate,
            limits,
        )
    }

    /// Allocation-free adjacent re-census after a fresh affine plan has been
    /// consumed and discarded. The supplied certificate must already have
    /// been authenticated by its owning V2 transaction; this method neither
    /// replays it nor performs native polynomial algebra.
    pub(crate) fn recompute_residual_affine_composition_plan_structural_census(
        &self,
        certificate: &ResidualAffineIntegerSystemCertificate,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<
        (
            ResidualAffineCompositionPlanStats,
            ResidualAffineCompositionPlanLogicalMemoryCensus,
        ),
        ResidualUnitAffineCompositionError,
    > {
        residual_affine_composition_plan_structural_census(self, certificate, limits)
    }

    fn compile_residual_affine_composition_plan_from_authenticated_integer_system(
        &self,
        certificate: Arc<ResidualAffineIntegerSystemCertificate>,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<ResidualAffineCompositionPlan, ResidualUnitAffineCompositionError> {
        if certificate.ambient_arity() != self.index_count() {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: self.index_count(),
                actual: certificate.ambient_arity(),
            });
        }
        let map = certificate.affine_map().ok_or_else(|| {
            ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                "affine-composition certificate proves an empty system",
            )
        })?;
        let geometry = compact_integer_system_affine_geometry(
            map,
            self.index_count(),
            self.base.variables().len(),
            limits,
        )?;
        let core = Arc::new(self.compile_residual_affine_composition_core(geometry, limits)?);
        let stats = core.stats;
        Ok(ResidualAffineCompositionPlan {
            schema: RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA,
            context_fingerprint: self.fingerprint.clone(),
            certificate,
            core,
            limits,
            stats,
        })
    }

    fn compile_residual_affine_composition_core(
        &self,
        geometry: ResidualAffineCompactGeometry,
        limits: ResidualUnitAffineCompositionPlanLimits,
    ) -> Result<ResidualAffineCompositionCorePlan, ResidualUnitAffineCompositionError> {
        let base_count = self.base.variables().len();
        let variable_count = base_count.checked_add(geometry.ambient_arity).ok_or(
            ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "composition variables",
            },
        )?;
        check_residual_affine_limit(
            "composition variables",
            variable_count,
            limits.max_variables,
        )?;
        check_residual_affine_limit("full-point images", variable_count, limits.max_full_images)?;

        let linear_support_count = residual_affine_checked_mul(
            "affine linear support entries retained",
            geometry.ambient_arity,
            geometry.free_positions.len(),
        )?;
        let support_entries_retained = residual_affine_checked_add(
            "affine support entries retained",
            geometry.ambient_arity,
            linear_support_count,
        )?;
        check_residual_affine_limit(
            "affine support entries retained",
            support_entries_retained,
            limits.max_support_entries_retained,
        )?;
        let mut linear_support = Vec::new();
        linear_support
            .try_reserve_exact(linear_support_count)
            .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
                resource: "affine linear support",
                requested: linear_support_count,
            })?;
        for coefficient in &geometry.linear_coefficients {
            linear_support.push(!coefficient.is_zero());
        }
        if linear_support.len() != linear_support_count {
            return Err(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "affine linear support",
                    actual: linear_support.len(),
                    bound: linear_support_count,
                },
            );
        }

        let mut image_term_counts = Vec::new();
        image_term_counts
            .try_reserve_exact(variable_count)
            .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
                resource: "image term counts",
                requested: variable_count,
            })?;
        let mut image_coefficient_growth_bits = Vec::new();
        image_coefficient_growth_bits
            .try_reserve_exact(variable_count)
            .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
                resource: "image coefficient growth bounds",
                requested: variable_count,
            })?;

        for _ in 0..base_count {
            image_term_counts.push(1);
            image_coefficient_growth_bits.push(0);
        }

        let mut total_image_terms = base_count;
        let total_image_integer_bits = geometry.total_image_integer_bits;
        let largest_image_integer_bits = geometry.largest_image_integer_bits;
        for position in 0..geometry.ambient_arity {
            let constant = geometry.constants.get(position).ok_or(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "compact affine constants",
                    actual: geometry.constants.len(),
                    bound: geometry.ambient_arity,
                },
            )?;
            let mut term_count = usize::from(!constant.is_zero());
            let mut growth_bits = residual_affine_integer_growth_bits(constant)?;
            for free_ordinal in 0..geometry.free_positions.len() {
                let coefficient = geometry.linear_coefficient(position, free_ordinal).ok_or(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "compact affine linear coefficients",
                        actual: geometry.linear_coefficients.len(),
                        bound: geometry.geometry_entries_retained,
                    },
                )?;
                if !coefficient.is_zero() {
                    term_count = residual_affine_checked_add("image terms", term_count, 1)?;
                    growth_bits =
                        growth_bits.max(residual_affine_integer_growth_bits(coefficient)?);
                }
            }
            total_image_terms =
                residual_affine_checked_add("total image terms", total_image_terms, term_count)?;
            check_residual_affine_limit(
                "total image terms",
                total_image_terms,
                limits.max_total_image_terms,
            )?;
            image_term_counts.push(term_count);
            image_coefficient_growth_bits.push(growth_bits);
        }

        let total_image_exponent_entries = residual_affine_checked_mul(
            "total image exponent entries",
            total_image_terms,
            variable_count,
        )?;
        check_residual_affine_limit(
            "total image exponent entries",
            total_image_exponent_entries,
            limits.max_total_image_exponent_entries,
        )?;

        let mut full_images = Vec::new();
        full_images.try_reserve_exact(variable_count).map_err(|_| {
            ResidualUnitAffineCompositionError::AllocationFailure {
                resource: "full-point images",
                requested: variable_count,
            }
        })?;
        let mut exponents = Vec::new();
        exponents.try_reserve_exact(variable_count).map_err(|_| {
            ResidualUnitAffineCompositionError::AllocationFailure {
                resource: "image exponent scratch",
                requested: variable_count,
            }
        })?;
        exponents.resize(variable_count, 0_u16);

        for variable in 0..base_count {
            exponents.fill(0);
            exponents[variable] = 1;
            let mut image = reserve_residual_affine_polynomial(
                &self.template.numerator,
                1,
                variable_count,
                "base identity image",
            )?;
            image.append_monomial(Integer::one(), &exponents);
            full_images.push(image);
        }
        for position in 0..geometry.ambient_arity {
            let term_count = image_term_counts[base_count + position];
            let mut image = reserve_residual_affine_polynomial(
                &self.template.numerator,
                term_count,
                variable_count,
                "index affine image",
            )?;
            let constant = &geometry.constants[position];
            if !constant.is_zero() {
                exponents.fill(0);
                image.append_monomial(constant.clone(), &exponents);
            }
            for (free_ordinal, &free_position) in geometry.free_positions.iter().enumerate() {
                let coefficient = geometry.linear_coefficient(position, free_ordinal).ok_or(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "compact affine linear coefficients",
                        actual: geometry.linear_coefficients.len(),
                        bound: geometry.geometry_entries_retained,
                    },
                )?;
                if coefficient.is_zero() {
                    continue;
                }
                exponents.fill(0);
                exponents[base_count + free_position] = 1;
                image.append_monomial(coefficient.clone(), &exponents);
            }
            full_images.push(image);
        }

        if full_images.len() != variable_count
            || image_term_counts.len() != variable_count
            || image_coefficient_growth_bits.len() != variable_count
        {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: variable_count,
                actual: full_images.len(),
            });
        }
        let authentication_limits = ExactAlgebraLimits {
            max_exponent: 1,
            max_polynomial_terms: limits.max_total_image_terms,
            max_term_operations: limits.max_total_image_terms,
        };
        for image in &full_images {
            if !Arc::ptr_eq(&image.variables, &self.variables) {
                return Err(ResidualUnitAffineCompositionError::WrongContext);
            }
            validate_polynomial_on_map(
                image,
                &self.variables,
                crate::CoefficientPolynomialPart::Numerator,
                authentication_limits,
            )
            .map_err(ParametricCoefficientError::from)?;
        }

        let stats = ResidualAffineCompositionPlanStats {
            variables: variable_count,
            full_images: full_images.len(),
            geometry_entries_inspected: geometry.geometry_entries_inspected,
            geometry_entries_retained: geometry.geometry_entries_retained,
            support_entries_retained,
            total_image_terms,
            total_image_exponent_entries,
            largest_image_integer_bits,
            total_image_integer_bits,
        };
        Ok(ResidualAffineCompositionCorePlan {
            schema: RESIDUAL_AFFINE_COMPOSITION_CORE_V1_SCHEMA,
            context_fingerprint: self.fingerprint.clone(),
            ambient_arity: geometry.ambient_arity,
            free_positions: geometry.free_positions,
            nonfree_positions: geometry.nonfree_positions,
            linear_support,
            full_images,
            image_term_counts,
            image_coefficient_growth_bits,
            limits,
            stats,
        })
    }

    pub(crate) fn compose_polynomial_on_residual_unit_affine_map(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualUnitAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffinePolynomialComposition, ResidualUnitAffineCompositionError> {
        self.validate_residual_unit_affine_plan(plan)?;
        let preflight =
            self.preflight_residual_affine_polynomial_core(source, &plan.core, limits)?;
        self.execute_residual_affine_polynomial_core(source, &plan.core, limits, preflight)
    }

    /// Allocation-free preflight for one authority-neutral guard polynomial.
    /// Guard truth classification and provenance remain with the authority
    /// owner; this algebra layer only returns the exact mapped polynomial.
    pub(crate) fn preflight_guard_on_residual_affine_compact_composition_plan(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualAffineCompactCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffinePolynomialCompositionStats, ResidualUnitAffineCompositionError>
    {
        Ok(self
            .prepare_guard_on_residual_affine_compact_composition_plan(source, plan, limits)?
            .stats())
    }

    /// Preflight one compact-plan guard exactly once and retain the sealed
    /// Symbolica execution input. The returned token borrows the exact
    /// context, source, and plan and must be consumed to execute.
    pub(crate) fn prepare_guard_on_residual_affine_compact_composition_plan<'prepared>(
        &'prepared self,
        source: &'prepared ParametricPolynomial,
        plan: &'prepared ResidualAffineCompactCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<
        PreparedResidualAffineCompactGuardComposition<'prepared>,
        ResidualUnitAffineCompositionError,
    > {
        self.validate_residual_affine_compact_composition_plan(plan)?;
        let preflight =
            self.preflight_residual_affine_polynomial_core(source, &plan.core, limits)?;
        Ok(PreparedResidualAffineCompactGuardComposition {
            context: self,
            source,
            plan,
            limits,
            preflight,
        })
    }

    /// Compose one authority-neutral guard through the preflight-selected
    /// simultaneous Symbolica backend.
    pub(crate) fn compose_guard_on_residual_affine_compact_composition_plan(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualAffineCompactCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffinePolynomialComposition, ResidualUnitAffineCompositionError> {
        self.prepare_guard_on_residual_affine_compact_composition_plan(source, plan, limits)?
            .execute()
    }

    /// Preflight numerator, denominator, durable denominator, and
    /// normalization work without evaluating either coefficient half.
    pub(crate) fn preflight_coefficient_on_residual_affine_compact_composition_plan(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualAffineCompactCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientCompositionPreflight, ResidualUnitAffineCompositionError>
    {
        Ok(self
            .prepare_coefficient_on_residual_affine_compact_composition_plan(source, plan, limits)?
            .stats())
    }

    /// Preflight one compact-plan rational coefficient exactly once and retain
    /// both Symbolica polynomial preflights for later consuming execution.
    pub(crate) fn prepare_coefficient_on_residual_affine_compact_composition_plan<'prepared>(
        &'prepared self,
        source: &'prepared ParametricCoefficient,
        plan: &'prepared ResidualAffineCompactCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<
        PreparedResidualAffineCompactCoefficientComposition<'prepared>,
        ResidualUnitAffineCompositionError,
    > {
        self.validate_residual_affine_compact_composition_plan(plan)?;
        let preflight =
            self.prepare_residual_affine_coefficient_core(source, &plan.core, limits)?;
        Ok(PreparedResidualAffineCompactCoefficientComposition {
            context: self,
            source,
            plan,
            limits,
            preflight,
        })
    }

    /// Compose a rational coefficient without manufacturing guard provenance.
    /// The exact pre-normalization denominator is retained for the caller's
    /// authority-aware zero/nonzero classification.
    pub(crate) fn compose_coefficient_on_residual_affine_compact_composition_plan(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualAffineCompactCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientComposition, ResidualUnitAffineCompositionError> {
        self.prepare_coefficient_on_residual_affine_compact_composition_plan(source, plan, limits)?
            .execute()
    }

    /// Compose one polynomial through the source-neutral integer-system plan.
    /// Guard classification and provenance deliberately remain the caller's
    /// responsibility.
    pub(crate) fn preflight_polynomial_on_residual_affine_composition_plan(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffinePolynomialCompositionStats, ResidualUnitAffineCompositionError>
    {
        self.validate_residual_affine_composition_plan(plan)?;
        Ok(self
            .preflight_residual_affine_polynomial_core(source, &plan.core, limits)?
            .stats)
    }

    /// Compose one polynomial through the source-neutral integer-system plan.
    /// Guard classification and provenance deliberately remain the caller's
    /// responsibility.
    pub(crate) fn compose_polynomial_on_residual_affine_composition_plan(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffinePolynomialComposition, ResidualUnitAffineCompositionError> {
        self.validate_residual_affine_composition_plan(plan)?;
        let preflight =
            self.preflight_residual_affine_polynomial_core(source, &plan.core, limits)?;
        self.execute_residual_affine_polynomial_core(source, &plan.core, limits, preflight)
    }

    /// Preflight both halves of one rational coefficient without evaluating
    /// either half.  The denominator receives the remaining per-coefficient
    /// allowance after the numerator, exactly as in the executing path.
    pub(crate) fn preflight_coefficient_on_residual_affine_composition_plan(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientCompositionPreflight, ResidualUnitAffineCompositionError>
    {
        self.validate_residual_affine_composition_plan(plan)?;
        self.preflight_residual_affine_coefficient_core(source, &plan.core, limits)
    }

    fn preflight_residual_affine_coefficient_core(
        &self,
        source: &ParametricCoefficient,
        core: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientCompositionPreflight, ResidualUnitAffineCompositionError>
    {
        Ok(self
            .prepare_residual_affine_coefficient_core(source, core, limits)?
            .stats)
    }

    fn prepare_residual_affine_coefficient_core(
        &self,
        source: &ParametricCoefficient,
        core: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientCorePreflight, ResidualUnitAffineCompositionError> {
        self.validate_with_limits(source, limits.exact_algebra)?;
        let numerator = self.preflight_residual_unit_affine_polynomial_raw(
            &source.raw.numerator,
            core,
            limits,
        )?;
        let denominator_limits = residual_affine_remaining_limits(limits, &numerator)?;
        let denominator = self.preflight_residual_unit_affine_polynomial_raw(
            &source.raw.denominator,
            core,
            denominator_limits,
        )?;
        let aggregate = merge_residual_affine_stats(numerator.stats, denominator.stats)?;
        let durable_denominator_term_bound = denominator.stats.expanded_contribution_bound;
        let durable_denominator_exponent_entry_bound =
            denominator.stats.output_exponent_entry_bound;
        let durable_denominator_integer_bit_payload_bound = residual_affine_checked_mul(
            "durable denominator integer-bit payload bound",
            durable_denominator_term_bound,
            denominator.stats.largest_integer_coefficient_bit_bound,
        )?;
        let normalization_input_term_pair_bound = residual_affine_checked_mul(
            "coefficient normalization input term-pair bound",
            numerator.stats.expanded_contribution_bound.max(1),
            denominator.stats.expanded_contribution_bound,
        )?;
        check_residual_affine_limit(
            "coefficient normalization input term-pair bound",
            normalization_input_term_pair_bound,
            limits.max_normalization_input_term_pairs,
        )?;
        check_residual_affine_limit(
            "coefficient exact-algebra normalization input term-pair bound",
            normalization_input_term_pair_bound,
            limits.exact_algebra.max_term_operations,
        )?;
        let total_integer_bit_work_bound = residual_affine_checked_add(
            "coefficient total integer-bit work bound",
            aggregate.integer_bit_work_bound,
            durable_denominator_integer_bit_payload_bound,
        )?;
        check_residual_affine_limit(
            "coefficient total integer-bit work bound",
            total_integer_bit_work_bound,
            limits.max_integer_bit_work,
        )?;
        let stats = ResidualAffineCoefficientCompositionPreflight {
            numerator: numerator.stats,
            denominator: denominator.stats,
            aggregate,
            durable_denominator_term_bound,
            durable_denominator_exponent_entry_bound,
            durable_denominator_integer_bit_payload_bound,
            normalization_input_term_pair_bound,
            total_integer_bit_work_bound,
        };
        Ok(ResidualAffineCoefficientCorePreflight {
            numerator,
            denominator_limits,
            denominator,
            stats,
        })
    }

    /// Compose a rational coefficient through a source-neutral residual
    /// integer-system plan.
    ///
    /// The returned available value owns a separately preflighted copy of the
    /// mapped denominator before rational normalization.  This layer does not
    /// classify the denominator and deliberately creates no `GuardOrigin`;
    /// the proof-bearing branch or relation which consumes the result owns
    /// that policy.
    pub(crate) fn compose_coefficient_on_residual_affine_composition_plan(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientComposition, ResidualUnitAffineCompositionError> {
        self.validate_residual_affine_composition_plan(plan)?;
        self.compose_coefficient_on_residual_affine_core(source, &plan.core, limits)
    }

    fn compose_coefficient_on_residual_affine_core(
        &self,
        source: &ParametricCoefficient,
        core: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientComposition, ResidualUnitAffineCompositionError> {
        let preflight = self.prepare_residual_affine_coefficient_core(source, core, limits)?;
        self.execute_prepared_coefficient_on_residual_affine_core(source, core, limits, preflight)
    }

    fn execute_prepared_coefficient_on_residual_affine_core(
        &self,
        source: &ParametricCoefficient,
        core: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
        preflight: ResidualAffineCoefficientCorePreflight,
    ) -> Result<ResidualAffineCoefficientComposition, ResidualUnitAffineCompositionError> {
        let ResidualAffineCoefficientHalves {
            numerator: mapped_numerator,
            denominator: mapped_denominator,
            mut stats,
        } = self
            .execute_prepared_residual_affine_coefficient_halves(source, core, limits, preflight)?;
        if mapped_denominator.value.is_zero() {
            return Ok(ResidualAffineCoefficientComposition::ZeroMappedDenominator { stats });
        }

        // Preflight normalization before cloning any GMP denominator payload.
        // `max(1, num_terms)` is intentional: normalization of 0/Q can still
        // inspect and normalize Q.
        stats.normalization_input_term_pairs = self
            .preflight_residual_affine_coefficient_normalization(
                &mapped_numerator.value,
                &mapped_denominator.value,
                limits,
            )?;

        // Normalization consumes both mapped halves, so retain the exact
        // original denominator first.  The copy helper scans and bounds every
        // sparse/GMP allocation before cloning the first coefficient.
        let mut copy_limits = limits;
        copy_limits.max_integer_bit_work = limits
            .max_integer_bit_work
            .checked_sub(stats.aggregate.integer_bit_work_bound)
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "durable denominator integer-bit payload",
            })?;
        let durable_denominator = self
            .copy_residual_unit_affine_guard_polynomial(&mapped_denominator.value, copy_limits)?;
        stats.durable_guard_terms = durable_denominator.terms;
        stats.durable_guard_exponent_entries = durable_denominator.exponent_entries;
        stats.durable_guard_integer_bit_payload = durable_denominator.integer_bit_payload;
        stats.total_integer_bit_work_bound = residual_affine_checked_add(
            "coefficient total integer bit work",
            stats.aggregate.integer_bit_work_bound,
            durable_denominator.integer_bit_payload,
        )?;
        check_residual_affine_limit(
            "coefficient total integer bit work",
            stats.total_integer_bit_work_bound,
            limits.max_integer_bit_work,
        )?;

        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<
                IntegerRing,
                IntegerRing,
                u16,
            >>::from_num_den(
                mapped_numerator.value.raw,
                mapped_denominator.value.raw,
                &Z,
                true,
            )
        }))
        .map_err(|_| ResidualUnitAffineCompositionError::SymbolicaPanic {
            stage: "residual-affine coefficient normalization",
        })?;
        let value = self.wrap_checked_with_limits(raw, limits.exact_algebra)?;
        Ok(ResidualAffineCoefficientComposition::Available(
            ResidualAffineComposedCoefficient {
                value,
                mapped_denominator: durable_denominator.value,
                stats,
            },
        ))
    }

    pub(crate) fn compose_coefficient_on_residual_unit_affine_map(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualUnitAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffineCoefficientComposition, ResidualUnitAffineCompositionError> {
        self.validate_residual_unit_affine_plan(plan)?;
        let ResidualAffineCoefficientHalves {
            numerator: mapped_numerator,
            denominator: mapped_denominator,
            mut stats,
        } = self.compose_residual_affine_coefficient_halves(source, &plan.core, limits)?;
        if mapped_denominator.value.is_zero() {
            return Ok(ResidualUnitAffineCoefficientComposition::ZeroMappedDenominator { stats });
        }

        stats.normalization_input_term_pairs = self
            .preflight_residual_affine_coefficient_normalization(
                &mapped_numerator.value,
                &mapped_denominator.value,
                limits,
            )?;

        // Preserve the mapped pre-normalization denominator even when the
        // numerator is zero or Symbolica cancels the entire denominator.
        let mapped_denominator_guard = if mapped_denominator.value.is_nonzero_constant() {
            None
        } else {
            check_residual_affine_limit(
                "unit-affine coefficient denominator guard origins",
                2,
                limits.max_guard_origins,
            )?;
            let locator = residual_affine_guard_locator(&plan.map);
            let denominator_origin =
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator {
                    source_case: locator.0,
                    predicate_ordinal: locator.1,
                    bound_position: locator.2,
                };
            let affine_origin = GuardOrigin::ResidualUnitAffineIndexSubstitution {
                source_case: locator.0,
                predicate_ordinal: locator.1,
                bound_position: locator.2,
            };
            stats.durable_guard_origin_retained_bytes = residual_affine_guard_origin_copy_bytes(
                std::iter::empty(),
                [&denominator_origin, &affine_origin],
            )?;
            check_residual_affine_limit(
                "unit-affine coefficient denominator guard origin retained bytes",
                stats.durable_guard_origin_retained_bytes,
                limits.max_guard_origin_retained_bytes,
            )?;
            // The mapped denominator's term and exponent allocation was
            // authenticated above.  Copy it through an explicitly reserved
            // sparse payload so vector allocation failure remains typed.
            let mut copy_limits = limits;
            copy_limits.max_integer_bit_work = limits
                .max_integer_bit_work
                .checked_sub(stats.aggregate.integer_bit_work_bound)
                .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                    resource: "durable denominator integer-bit payload",
                })?;
            let durable_denominator = self.copy_residual_unit_affine_guard_polynomial(
                &mapped_denominator.value,
                copy_limits,
            )?;
            stats.durable_guard_terms = durable_denominator.terms;
            stats.durable_guard_exponent_entries = durable_denominator.exponent_entries;
            stats.durable_guard_integer_bit_payload = durable_denominator.integer_bit_payload;
            stats.total_integer_bit_work_bound = residual_affine_checked_add(
                "coefficient total integer bit work",
                stats.aggregate.integer_bit_work_bound,
                durable_denominator.integer_bit_payload,
            )?;
            check_residual_affine_limit(
                "coefficient total integer bit work",
                stats.total_integer_bit_work_bound,
                limits.max_integer_bit_work,
            )?;
            Some(self.nonzero_condition_with_origins_and_origin_limit(
                durable_denominator.value,
                [denominator_origin, affine_origin],
                limits.exact_algebra,
                limits.max_guard_origins,
            )?)
        };

        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<
                IntegerRing,
                IntegerRing,
                u16,
            >>::from_num_den(
                mapped_numerator.value.raw,
                mapped_denominator.value.raw,
                &Z,
                true,
            )
        }))
        .map_err(|_| ResidualUnitAffineCompositionError::SymbolicaPanic {
            stage: "unit-affine coefficient normalization",
        })?;
        let value = self.wrap_checked_with_limits(raw, limits.exact_algebra)?;
        Ok(ResidualUnitAffineCoefficientComposition::Available(
            GuardedResidualUnitAffineCoefficientComposition {
                value,
                mapped_denominator: mapped_denominator_guard,
                stats,
            },
        ))
    }

    /// Shared provenance-free numerator/denominator composition core used by
    /// both the legacy guarded adapter and the integer-system adapter.
    ///
    /// Both raw halves are preflighted before either Symbolica backend is
    /// entered. The denominator receives the exact remaining aggregate
    /// allowance after the numerator's prospective census.
    fn compose_residual_affine_coefficient_halves(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualAffineCoefficientHalves, ResidualUnitAffineCompositionError> {
        let preflight = self.prepare_residual_affine_coefficient_core(source, plan, limits)?;
        self.execute_prepared_residual_affine_coefficient_halves(source, plan, limits, preflight)
    }

    fn execute_prepared_residual_affine_coefficient_halves(
        &self,
        source: &ParametricCoefficient,
        plan: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
        preflight: ResidualAffineCoefficientCorePreflight,
    ) -> Result<ResidualAffineCoefficientHalves, ResidualUnitAffineCompositionError> {
        let ResidualAffineCoefficientCorePreflight {
            numerator: numerator_preflight,
            denominator_limits,
            denominator: denominator_preflight,
            stats: _,
        } = preflight;

        let numerator = self.execute_residual_unit_affine_polynomial_raw(
            &source.raw.numerator,
            plan,
            limits,
            numerator_preflight,
        )?;
        let denominator = self.execute_residual_unit_affine_polynomial_raw(
            &source.raw.denominator,
            plan,
            denominator_limits,
            denominator_preflight,
        )?;
        let numerator_stats = numerator.stats;
        let denominator_stats = denominator.stats;
        let aggregate = merge_residual_affine_stats(numerator_stats, denominator_stats)?;
        Ok(ResidualAffineCoefficientHalves {
            numerator,
            denominator,
            stats: ResidualUnitAffineCoefficientCompositionStats {
                numerator: numerator_stats,
                denominator: denominator_stats,
                aggregate,
                durable_guard_terms: 0,
                durable_guard_exponent_entries: 0,
                durable_guard_integer_bit_payload: 0,
                durable_guard_origin_retained_bytes: 0,
                total_integer_bit_work_bound: aggregate.integer_bit_work_bound,
                normalization_input_term_pairs: 0,
            },
        })
    }

    fn preflight_residual_affine_coefficient_normalization(
        &self,
        mapped_numerator: &ParametricPolynomial,
        mapped_denominator: &ParametricPolynomial,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<usize, ResidualUnitAffineCompositionError> {
        let normalization_input_term_pairs = residual_affine_checked_mul(
            "coefficient normalization input term pairs",
            mapped_numerator.raw.nterms().max(1),
            mapped_denominator.raw.nterms(),
        )?;
        check_residual_affine_limit(
            "coefficient normalization input term pairs",
            normalization_input_term_pairs,
            limits.max_normalization_input_term_pairs,
        )?;
        check_residual_affine_limit(
            "coefficient exact-algebra normalization input term pairs",
            normalization_input_term_pairs,
            limits.exact_algebra.max_term_operations,
        )?;
        Ok(normalization_input_term_pairs)
    }

    pub(crate) fn compose_nonzero_condition_on_residual_unit_affine_map(
        &self,
        source: &ParametricNonZeroCondition,
        plan: &ResidualUnitAffineCompositionPlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffineConditionComposition, ResidualUnitAffineCompositionError> {
        self.validate_residual_unit_affine_plan(plan)?;
        if !self.contains_nonzero_condition(source) {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        let mapped =
            self.compose_polynomial_on_residual_unit_affine_map(source.polynomial(), plan, limits)?;
        let stats = mapped.stats;
        if mapped.value.is_zero() {
            return Ok(ResidualUnitAffineConditionComposition {
                class: ResidualUnitAffineConditionClass::Unsatisfiable,
                stats,
                guard_origin_retained_bytes: 0,
            });
        }
        if mapped.value.is_nonzero_constant() {
            return Ok(ResidualUnitAffineConditionComposition {
                class: ResidualUnitAffineConditionClass::NonzeroIntegerConstant,
                stats,
                guard_origin_retained_bytes: 0,
            });
        }

        let locator = residual_affine_guard_locator(&plan.map);
        let affine_origin = GuardOrigin::ResidualUnitAffineIndexSubstitution {
            source_case: locator.0,
            predicate_ordinal: locator.1,
            bound_position: locator.2,
        };
        let needs_origin = !source.origins.contains(&affine_origin);
        let requested = source
            .origins
            .len()
            .checked_add(usize::from(needs_origin))
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "unit-affine guard origins",
            })?;
        check_residual_affine_limit(
            "unit-affine guard origins",
            requested,
            limits.max_guard_origins,
        )?;
        let provenance_bytes = residual_affine_guard_origin_copy_bytes(
            source.origins.iter(),
            std::iter::once(&affine_origin).filter(|_| needs_origin),
        )?;
        check_residual_affine_limit(
            "unit-affine guard origin retained bytes",
            provenance_bytes,
            limits.max_guard_origin_retained_bytes,
        )?;
        // Clone only after the complete provenance-cardinality preflight.
        let mut origins = source.origins.clone();
        if needs_origin {
            origins.insert(affine_origin);
        }
        let index_dependent =
            self.polynomial_depends_on_indices_with_limits(&mapped.value, limits.exact_algebra)?;
        let condition = self.nonzero_condition_with_origins_and_origin_limit(
            mapped.value,
            origins,
            limits.exact_algebra,
            limits.max_guard_origins,
        )?;
        Ok(ResidualUnitAffineConditionComposition {
            class: if index_dependent {
                ResidualUnitAffineConditionClass::IndexDependent(condition)
            } else {
                ResidualUnitAffineConditionClass::BaseAssumption(condition)
            },
            stats,
            guard_origin_retained_bytes: provenance_bytes,
        })
    }

    /// Copy one already-composed polynomial into durable guard storage.
    ///
    /// The Symbolica polynomial's two backing vectors are reserved fallibly
    /// before any coefficient is cloned. Large-integer payload is bounded by
    /// the same aggregate integer-bit allowance used for selected Symbolica
    /// composition;
    /// Rust's allocator still cannot make an individual GMP limb clone
    /// intrinsically fallible, but no unbounded vector growth is hidden in a
    /// derived `Clone` call.
    fn copy_residual_unit_affine_guard_polynomial(
        &self,
        source: &ParametricPolynomial,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffineGuardPolynomialCopy, ResidualUnitAffineCompositionError> {
        self.validate_polynomial_with_limits(source, limits.exact_algebra)?;
        let terms = source.raw.nterms();
        check_residual_affine_limit("durable denominator terms", terms, limits.max_output_terms)?;
        let exponent_entries = residual_affine_checked_mul(
            "durable denominator exponent entries",
            terms,
            self.variables.len(),
        )?;
        check_residual_affine_limit(
            "durable denominator exponent entries",
            exponent_entries,
            limits.max_output_exponent_entries,
        )?;

        let mut integer_bit_payload = 0usize;
        for coefficient in &source.raw.coefficients {
            let bits = residual_affine_integer_bits(coefficient)?;
            check_residual_affine_limit(
                "durable denominator integer coefficient bits",
                bits,
                limits.max_integer_coefficient_bits,
            )?;
            integer_bit_payload = residual_affine_checked_add(
                "durable denominator integer-bit payload",
                integer_bit_payload,
                bits,
            )?;
        }
        check_residual_affine_limit(
            "durable denominator integer-bit payload",
            integer_bit_payload,
            limits.max_integer_bit_work,
        )?;

        let mut raw = reserve_residual_affine_polynomial(
            &source.raw,
            terms,
            self.variables.len(),
            "durable denominator sparse payload",
        )?;
        for term in &source.raw {
            raw.append_monomial(term.coefficient.clone(), term.exponents);
        }
        Ok(ResidualUnitAffineGuardPolynomialCopy {
            value: ParametricPolynomial {
                raw,
                context: source.context.clone(),
            },
            terms,
            exponent_entries,
            integer_bit_payload,
        })
    }

    fn validate_residual_unit_affine_plan(
        &self,
        plan: &ResidualUnitAffineCompositionPlan,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        if plan.schema != RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        if plan.context_fingerprint.as_ref() != self.fingerprint.as_ref()
            || plan.map.context_fingerprint() != self.fingerprint()
        {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        if plan.limits != plan.core.limits {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        self.validate_residual_affine_composition_core(&plan.core)
    }

    fn validate_residual_affine_compact_composition_plan(
        &self,
        plan: &ResidualAffineCompactCompositionPlan,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        if plan.schema != RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA
            || plan.manifest.schema != RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA
            || plan.core.schema != RESIDUAL_AFFINE_COMPOSITION_CORE_V1_SCHEMA
        {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        if plan.context_fingerprint.as_ref() != self.fingerprint() {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        if plan.limits.composition != plan.core.limits
            || plan.stats.composition != plan.core.stats
            || plan.manifest.limits != plan.limits
            || plan.manifest.stats != plan.stats
            || plan.manifest.context_fingerprint_bytes != plan.context_fingerprint.len()
            || plan.manifest.context_checksum
                != residual_affine_diagnostic_checksum(plan.context_fingerprint.as_bytes())
            || plan.manifest.ambient_arity != plan.core.ambient_arity
            || plan.manifest.free_count != plan.core.free_positions.len()
            || plan.manifest.geometry_checksum != plan.geometry_checksum
        {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        check_residual_affine_limit(
            "compact affine context fingerprint bytes",
            plan.context_fingerprint.len(),
            plan.limits.max_context_fingerprint_bytes,
        )?;
        check_residual_affine_limit(
            "compact affine geometry integer bit work",
            plan.stats.geometry_integer_bit_work,
            plan.limits.max_geometry_integer_bit_work,
        )?;
        check_residual_affine_limit(
            "compact affine replay comparison work",
            plan.stats.geometry_replay_comparison_work,
            plan.limits.max_geometry_replay_comparison_work,
        )?;
        check_residual_affine_limit(
            "compact affine replay integer bit work",
            plan.stats.geometry_replay_integer_bit_work,
            plan.limits.max_geometry_replay_integer_bit_work,
        )?;
        check_residual_affine_limit(
            "compact affine replay scratch logical bytes",
            plan.stats.geometry_replay_scratch_logical_bytes,
            plan.limits.max_geometry_replay_scratch_logical_bytes,
        )?;
        check_residual_affine_limit(
            "compact affine retained owned logical bytes",
            plan.stats.retained_owned_logical_bytes,
            plan.limits.max_retained_owned_logical_bytes,
        )?;
        check_residual_affine_limit(
            "compact affine compilation owned logical peak upper bound",
            plan.stats.compilation_owned_logical_peak_upper_bound,
            plan.limits.max_compilation_owned_logical_peak_upper_bound,
        )?;
        self.validate_residual_affine_composition_core(&plan.core)
    }

    fn replay_residual_affine_compact_composition_plan(
        &self,
        plan: &ResidualAffineCompactCompositionPlan,
        geometry: ResidualAffineCompactMapView<'_>,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.replay_residual_affine_compact_composition_plan_unwind_boundary(plan, geometry)
        }))
        .map_err(|_| ResidualUnitAffineCompositionError::SymbolicaPanic {
            stage: "compact affine composition plan replay",
        })?
    }

    fn replay_residual_affine_compact_composition_plan_unwind_boundary(
        &self,
        plan: &ResidualAffineCompactCompositionPlan,
        geometry: ResidualAffineCompactMapView<'_>,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        #[cfg(test)]
        maybe_inject_residual_affine_compact_boundary_panic_for_test();
        self.validate_residual_affine_compact_composition_plan(plan)?;
        let preflight = residual_affine_compact_geometry_preflight(self, geometry, plan.limits)?;
        let stats = ResidualAffineCompactCompositionPlanStats {
            composition: preflight.composition,
            context_fingerprint_bytes: self.fingerprint().len(),
            geometry_integer_entries_inspected: preflight.geometry_integer_entries_inspected,
            geometry_integer_bit_work: preflight.geometry_integer_bit_work,
            geometry_replay_comparison_work: preflight.geometry_replay_comparison_work,
            geometry_replay_integer_bit_work: preflight.geometry_replay_integer_bit_work,
            geometry_replay_scratch_logical_bytes: preflight.geometry_replay_scratch_logical_bytes,
            retained_owned_logical_bytes: preflight.retained_owned_logical_bytes,
            compilation_owned_logical_peak_upper_bound: preflight
                .compilation_owned_logical_peak_upper_bound,
        };
        let manifest = ResidualAffineCompactCompositionManifest {
            schema: RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA,
            context_fingerprint_bytes: self.fingerprint().len(),
            context_checksum: residual_affine_diagnostic_checksum(self.fingerprint().as_bytes()),
            ambient_arity: geometry.ambient_arity,
            free_count: geometry.free_positions.len(),
            geometry_checksum: preflight.geometry_checksum,
            limits: plan.limits,
            stats,
        };
        if plan.stats != stats || plan.manifest != manifest {
            return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
        }
        replay_residual_affine_compact_geometry_against_core(self, &plan.core, geometry, preflight)
    }

    fn validate_residual_affine_composition_plan(
        &self,
        plan: &ResidualAffineCompositionPlan,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        if plan.schema != RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        if plan.context_fingerprint.as_ref() != self.fingerprint.as_ref() {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        if plan.limits != plan.core.limits || plan.stats != plan.core.stats {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        if plan.certificate.ambient_arity() != self.index_count() {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: self.index_count(),
                actual: plan.certificate.ambient_arity(),
            });
        }
        self.validate_residual_affine_composition_core(&plan.core)
    }

    fn validate_residual_affine_composition_core(
        &self,
        plan: &ResidualAffineCompositionCorePlan,
    ) -> Result<(), ResidualUnitAffineCompositionError> {
        if plan.schema != RESIDUAL_AFFINE_COMPOSITION_CORE_V1_SCHEMA {
            return Err(ResidualUnitAffineCompositionError::SchemaMismatch);
        }
        if plan.context_fingerprint.as_ref() != self.fingerprint.as_ref() {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        if plan.ambient_arity != self.index_count() {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: self.index_count(),
                actual: plan.ambient_arity,
            });
        }
        let expected = self.variables.len();
        if plan.full_images.len() != expected
            || plan.image_term_counts.len() != expected
            || plan.image_coefficient_growth_bits.len() != expected
        {
            return Err(ResidualUnitAffineCompositionError::WrongArity {
                expected,
                actual: plan.full_images.len(),
            });
        }
        let expected_linear_support = residual_affine_checked_mul(
            "affine linear support",
            plan.ambient_arity,
            plan.free_positions.len(),
        )?;
        if plan.linear_support.len() != expected_linear_support
            || plan
                .free_positions
                .len()
                .checked_add(plan.nonfree_positions.len())
                != Some(plan.ambient_arity)
        {
            return Err(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "compact affine geometry",
                    actual: plan.linear_support.len(),
                    bound: expected_linear_support,
                },
            );
        }
        Ok(())
    }

    fn preflight_residual_affine_polynomial_core(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffinePolynomialPreflight, ResidualUnitAffineCompositionError> {
        self.validate_residual_affine_composition_core(plan)?;
        self.validate_polynomial_with_limits(source, limits.exact_algebra)?;
        self.preflight_residual_unit_affine_polynomial_raw(&source.raw, plan, limits)
    }

    fn preflight_residual_unit_affine_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        plan: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
    ) -> Result<ResidualUnitAffinePolynomialPreflight, ResidualUnitAffineCompositionError> {
        #[cfg(test)]
        note_residual_affine_compact_preflight_call_for_test();
        self.validate_residual_affine_composition_core(plan)?;
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )
        .map_err(ParametricCoefficientError::from)?;
        let source_terms = source.nterms();
        check_residual_affine_limit(
            "polynomial source terms",
            source_terms,
            limits.max_source_terms,
        )?;
        let variable_count = self.variables.len();
        let source_exponent_entries = residual_affine_checked_mul(
            "polynomial source exponent entries",
            source_terms,
            variable_count,
        )?;
        check_residual_affine_limit(
            "polynomial source exponent entries",
            source_exponent_entries,
            limits.max_source_exponent_entries,
        )?;

        let base_count = self.base.variables().len();
        let exponent_limit = limits
            .exact_algebra
            .max_exponent
            .min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT);
        let mut expanded_contribution_bound = 0usize;
        let mut power_calls = 0usize;
        let mut native_power_heap_pair_bound = 0usize;
        let mut multiplication_term_pair_bound = 0usize;
        let mut polynomial_evaluator_addition_term_visit_bound = 0usize;
        let mut accumulated_output_term_bound = 0usize;
        let mut accumulated_output_integer_bits = 0usize;
        let mut largest_kronecker_exponent_bits = 0usize;
        let mut largest_integer_contribution_bits = 0usize;
        let mut largest_polynomial_evaluator_integer_coefficient_bits = 0usize;
        let mut polynomial_evaluator_integer_bit_work_bound = 0usize;
        let mut backend = ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator;

        // Select the Symbolica compositor before computing either backend's
        // integer-work census.  In particular, an input that requires Atom
        // expansion must not be rejected by checked resource arithmetic that
        // belongs exclusively to `Polynomial::evaluate`/`heap_pow`.
        for source_term in 0..source_terms {
            for (variable, &exponent) in source.exponents(source_term).iter().enumerate() {
                if exponent == 0 {
                    continue;
                }
                let kronecker = residual_affine_kronecker_preflight(
                    plan,
                    variable,
                    usize::from(exponent),
                    base_count,
                )?;
                if !kronecker.polynomial_evaluator_safe {
                    backend =
                        ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion;
                }
                check_residual_affine_limit(
                    "Kronecker exponent bits",
                    kronecker.exponent_bits,
                    limits.max_kronecker_exponent_bits,
                )?;
                largest_kronecker_exponent_bits =
                    largest_kronecker_exponent_bits.max(kronecker.exponent_bits);
            }
        }
        let polynomial_evaluator_selected =
            backend == ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator;

        for source_term in 0..source_terms {
            let source_exponents = source.exponents(source_term);

            // Check every prospective target exponent before Symbolica can
            // multiply two u16 exponent vectors.  Free identity rows are
            // included once through the complete A matrix.
            for variable in 0..base_count {
                let requested = u128::from(source_exponents[variable]);
                check_residual_affine_exponent(source_term, variable, requested, exponent_limit)?;
            }
            for (free_ordinal, &free_position) in plan.free_positions.iter().enumerate() {
                let mut requested = 0u128;
                for position in 0..self.index_count() {
                    let coefficient_is_nonzero =
                        plan.linear_is_nonzero(position, free_ordinal).ok_or(
                            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                                resource: "affine linear support",
                                actual: plan.linear_support.len(),
                                bound: plan.ambient_arity.saturating_mul(plan.free_positions.len()),
                            },
                        )?;
                    if coefficient_is_nonzero {
                        requested = requested
                            .checked_add(u128::from(source_exponents[base_count + position]))
                            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                                resource: "target exponent",
                            })?;
                    }
                }
                check_residual_affine_exponent(
                    source_term,
                    base_count + free_position,
                    requested,
                    exponent_limit,
                )?;
            }

            let mut contribution_bound = 1usize;
            // Both Symbolica backends retain the source integer while
            // composing this term. This work is unconditional even when a
            // later zero image kills the term.
            let source_integer_bits =
                residual_affine_integer_bits(&source.coefficients[source_term])?;
            check_residual_affine_limit(
                "integer coefficient bits",
                source_integer_bits,
                limits.max_integer_coefficient_bits,
            )?;
            if polynomial_evaluator_selected {
                largest_polynomial_evaluator_integer_coefficient_bits =
                    largest_polynomial_evaluator_integer_coefficient_bits.max(source_integer_bits);
                polynomial_evaluator_integer_bit_work_bound = residual_affine_checked_add(
                    "polynomial evaluator integer bit work",
                    polynomial_evaluator_integer_bit_work_bound,
                    source_integer_bits,
                )?;
            }
            let mut term_integer_bits = source_integer_bits;
            let mut polynomial_evaluator_prefix_integer_bits = source_integer_bits;
            for (variable, &exponent) in source_exponents.iter().enumerate() {
                if exponent == 0 {
                    continue;
                }
                power_calls = residual_affine_checked_add("native power calls", power_calls, 1)?;
                check_residual_affine_limit(
                    "native power calls",
                    power_calls,
                    limits.max_power_calls,
                )?;

                let image_terms = plan.image_term_counts[variable];
                let power_terms = residual_affine_affine_power_term_bound(
                    usize::from(exponent),
                    image_terms,
                    limits
                        .max_expanded_contributions
                        .min(limits.max_output_terms)
                        .min(limits.exact_algebra.max_polynomial_terms),
                )?;
                let heap_pairs = residual_affine_checked_mul(
                    "native power heap pairs",
                    image_terms,
                    power_terms,
                )?;
                native_power_heap_pair_bound = residual_affine_checked_add(
                    "native power heap pairs",
                    native_power_heap_pair_bound,
                    heap_pairs,
                )?;
                check_residual_affine_limit(
                    "native power heap pairs",
                    native_power_heap_pair_bound,
                    limits.max_native_power_heap_pairs,
                )?;

                let multiplication_pairs = residual_affine_checked_mul(
                    "native multiplication term pairs",
                    contribution_bound,
                    power_terms,
                )?;

                if power_terms != 0 && polynomial_evaluator_selected {
                    let kronecker = residual_affine_kronecker_preflight(
                        plan,
                        variable,
                        usize::from(exponent),
                        base_count,
                    )?;
                    let per_power_growth = plan.image_coefficient_growth_bits[variable]
                        .checked_add(residual_affine_ceil_log2(image_terms))
                        .and_then(|bits| bits.checked_mul(usize::from(exponent)))
                        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                            resource: "integer coefficient growth bits",
                        })?;
                    // Every powered image is materialized before it is
                    // multiplied into the accumulating term, even if an
                    // earlier image was zero. One leading bit covers unit
                    // coefficients; the remaining expression bounds the
                    // final multinomial and affine-image coefficient growth.
                    let power_final_integer_bits = per_power_growth.checked_add(1).ok_or(
                        ResidualUnitAffineCompositionError::ResourceCountOverflow {
                            resource: "integer coefficient bits",
                        },
                    )?;
                    // `heap_pow` additionally multiplies by encoded
                    // Kronecker-exponent differences and accumulates up to
                    // w*H recurrence pairs. Bound those transient integers,
                    // not merely the final power coefficients.
                    let power_native_integer_bits = if image_terms > 1 && usize::from(exponent) > 1
                    {
                        power_final_integer_bits
                            .checked_add(
                                plan.image_coefficient_growth_bits[variable]
                                    .checked_add(1)
                                    .ok_or(
                                        ResidualUnitAffineCompositionError::ResourceCountOverflow {
                                            resource: "integer coefficient bits",
                                        },
                                    )?,
                            )
                            .and_then(|bits| bits.checked_add(kronecker.exponent_bits))
                            .and_then(|bits| {
                                bits.checked_add(residual_affine_ceil_log2(heap_pairs.max(1)))
                            })
                            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                                resource: "integer coefficient bits",
                            })?
                    } else {
                        power_final_integer_bits
                    };
                    largest_polynomial_evaluator_integer_coefficient_bits =
                        largest_polynomial_evaluator_integer_coefficient_bits
                            .max(power_native_integer_bits);
                    let power_work_units = if image_terms > 1 && usize::from(exponent) > 1 {
                        // One recurrence accumulation per heap pair plus one
                        // denominator-product/division step per retained
                        // power coefficient.
                        residual_affine_checked_add(
                            "native power integer work units",
                            heap_pairs,
                            power_terms,
                        )?
                    } else {
                        power_terms
                    };
                    let power_integer_bit_work = residual_affine_checked_mul(
                        "native power integer bit work",
                        power_work_units,
                        power_native_integer_bits,
                    )?;
                    polynomial_evaluator_integer_bit_work_bound = residual_affine_checked_add(
                        "polynomial evaluator integer bit work",
                        polynomial_evaluator_integer_bit_work_bound,
                        power_integer_bit_work,
                    )?;

                    // The evaluator next multiplies the accumulated term by
                    // the fully materialized power. Charge every prospective
                    // product coefficient before mutating the prefix. This
                    // remains necessary when a later image is identically
                    // zero and the final C_m is therefore zero.
                    let multiplication_integer_bits = polynomial_evaluator_prefix_integer_bits
                        .checked_add(power_final_integer_bits)
                        .and_then(|bits| {
                            bits.checked_add(residual_affine_ceil_log2(multiplication_pairs.max(1)))
                        })
                        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                            resource: "integer coefficient bits",
                        })?;
                    if multiplication_pairs != 0 {
                        largest_polynomial_evaluator_integer_coefficient_bits =
                            largest_polynomial_evaluator_integer_coefficient_bits
                                .max(multiplication_integer_bits);
                    }
                    let multiplication_integer_bit_work = residual_affine_checked_mul(
                        "native multiplication integer bit work",
                        multiplication_pairs,
                        multiplication_integer_bits,
                    )?;
                    polynomial_evaluator_integer_bit_work_bound = residual_affine_checked_add(
                        "polynomial evaluator integer bit work",
                        polynomial_evaluator_integer_bit_work_bound,
                        multiplication_integer_bit_work,
                    )?;
                    polynomial_evaluator_prefix_integer_bits = multiplication_integer_bits;
                    // This is also the bound on the collected coefficient of
                    // the current term prefix; carry multiplication
                    // collisions into later output-addition accounting.
                    term_integer_bits = multiplication_integer_bits;
                }

                multiplication_term_pair_bound = residual_affine_checked_add(
                    "native multiplication term pairs",
                    multiplication_term_pair_bound,
                    multiplication_pairs,
                )?;
                check_residual_affine_limit(
                    "native multiplication term pairs",
                    multiplication_term_pair_bound,
                    limits.max_multiplication_term_pairs,
                )?;
                contribution_bound = multiplication_pairs;
                check_residual_affine_limit(
                    "expanded polynomial contributions",
                    contribution_bound,
                    limits.max_expanded_contributions,
                )?;
            }

            expanded_contribution_bound = residual_affine_checked_add(
                "expanded polynomial contributions",
                expanded_contribution_bound,
                contribution_bound,
            )?;
            check_residual_affine_limit(
                "expanded polynomial contributions",
                expanded_contribution_bound,
                limits.max_expanded_contributions,
            )?;
            check_residual_affine_limit(
                "prospective output terms",
                expanded_contribution_bound,
                limits.max_output_terms,
            )?;
            check_residual_affine_limit(
                "prospective exact-algebra output terms",
                expanded_contribution_bound,
                limits.exact_algebra.max_polynomial_terms,
            )?;

            if polynomial_evaluator_selected {
                let addition_visits = residual_affine_checked_add(
                    "polynomial evaluator addition term visits",
                    expanded_contribution_bound,
                    contribution_bound,
                )?
                .checked_sub(contribution_bound)
                .ok_or(
                    ResidualUnitAffineCompositionError::ResourceCountOverflow {
                        resource: "polynomial evaluator addition term visits",
                    },
                )?;
                // The expression above is the new prefix plus C_m; spell the
                // equivalent old-prefix + C_m without retaining another
                // mutable prefix counter.
                polynomial_evaluator_addition_term_visit_bound = residual_affine_checked_add(
                    "polynomial evaluator addition term visits",
                    polynomial_evaluator_addition_term_visit_bound,
                    addition_visits,
                )?;
                check_residual_affine_limit(
                    "native addition term visits",
                    polynomial_evaluator_addition_term_visit_bound,
                    limits.max_addition_term_visits,
                )?;
            }

            if contribution_bound != 0 && polynomial_evaluator_selected {
                largest_integer_contribution_bits =
                    largest_integer_contribution_bits.max(term_integer_bits);

                // Owned polynomial addition walks/copies both sparse inputs
                // and performs integer additions at collisions. Charge a
                // conservative coefficient-width allowance for every visit
                // before updating the accumulated output prefix.
                let addition_integer_bits = accumulated_output_integer_bits
                    .max(term_integer_bits)
                    .checked_add(1)
                    .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                        resource: "integer coefficient bits",
                    })?;
                let addition_integer_visits = residual_affine_checked_add(
                    "native addition integer visits",
                    accumulated_output_term_bound,
                    contribution_bound,
                )?;
                let addition_integer_bit_work = residual_affine_checked_mul(
                    "native addition integer bit work",
                    addition_integer_visits,
                    addition_integer_bits,
                )?;
                polynomial_evaluator_integer_bit_work_bound = residual_affine_checked_add(
                    "polynomial evaluator integer bit work",
                    polynomial_evaluator_integer_bit_work_bound,
                    addition_integer_bit_work,
                )?;
                accumulated_output_integer_bits = addition_integer_bits;
            }
            accumulated_output_term_bound = expanded_contribution_bound;
        }

        let addition_term_visit_bound = match backend {
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator => {
                polynomial_evaluator_addition_term_visit_bound
            }
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion => {
                // The execution iterator filters inactive nonfree variables
                // without allocating a support set. Reproduce its exact
                // support predicate while counting the RHS Atoms that
                // Symbolica will collect internally.
                let (
                    active_replacement_count,
                    replacement_image_terms,
                    replacement_occurrence_image_terms,
                ) = plan.nonfree_positions.iter().try_fold(
                    (0usize, 0usize, 0usize),
                    |(active, terms, occurrence_terms),
                     &position|
                     -> Result<_, ResidualUnitAffineCompositionError> {
                        let variable = base_count.checked_add(position).ok_or(
                            ResidualUnitAffineCompositionError::ResourceCountOverflow {
                                resource: "Symbolica expression replacement image terms",
                            },
                        )?;
                        let occurrences =
                            (0..source_terms).try_fold(0usize, |occurrences, source_term| {
                                if source.exponents(source_term)[variable] == 0 {
                                    Ok(occurrences)
                                } else {
                                    residual_affine_checked_add(
                                        "Symbolica expression replacement occurrences",
                                        occurrences,
                                        1,
                                    )
                                }
                            })?;
                        if occurrences == 0 {
                            return Ok((active, terms, occurrence_terms));
                        }
                        let image_terms = plan.image_term_counts[variable];
                        Ok((
                            residual_affine_checked_add(
                                "Symbolica expression active replacements",
                                active,
                                1,
                            )?,
                            residual_affine_checked_add(
                                "Symbolica expression replacement image terms",
                                terms,
                                image_terms,
                            )?,
                            residual_affine_checked_add(
                                "Symbolica expression substituted image terms",
                                occurrence_terms,
                                residual_affine_checked_mul(
                                    "Symbolica expression substituted image terms",
                                    occurrences,
                                    image_terms,
                                )?,
                            )?,
                        ))
                    },
                )?;
                let bound = residual_affine_symbolica_expression_structural_visit_bound(
                    source_terms,
                    variable_count,
                    plan.nonfree_positions.len(),
                    active_replacement_count,
                    replacement_image_terms,
                    replacement_occurrence_image_terms,
                    native_power_heap_pair_bound,
                    multiplication_term_pair_bound,
                    expanded_contribution_bound,
                )?;
                check_residual_affine_limit(
                    "Symbolica backend structural term visits",
                    bound,
                    limits.max_addition_term_visits,
                )?;
                bound
            }
        };

        let prospective_output_exponents = residual_affine_checked_mul(
            "prospective output exponent entries",
            expanded_contribution_bound,
            variable_count,
        )?;
        check_residual_affine_limit(
            "prospective output exponent entries",
            prospective_output_exponents,
            limits.max_output_exponent_entries,
        )?;
        let mut selected_backend_integer = match backend {
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator => {
                ResidualAffineSymbolicaExpressionIntegerPreflight {
                    largest_integer_coefficient_bit_bound:
                        largest_polynomial_evaluator_integer_coefficient_bits,
                    largest_integer_contribution_bit_bound: largest_integer_contribution_bits,
                    integer_bit_work_bound: polynomial_evaluator_integer_bit_work_bound,
                }
            }
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion => {
                residual_affine_symbolica_expression_integer_preflight(
                    source,
                    plan,
                    base_count,
                    limits
                        .max_expanded_contributions
                        .min(limits.max_output_terms)
                        .min(limits.exact_algebra.max_polynomial_terms),
                )?
            }
        };
        let largest_output_integer_coefficient_bit_bound = if expanded_contribution_bound == 0 {
            0
        } else {
            selected_backend_integer
                .largest_integer_contribution_bit_bound
                .checked_add(residual_affine_ceil_log2(expanded_contribution_bound))
                .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                    resource: "integer coefficient bits",
                })?
        };
        let largest_integer_coefficient_bit_bound = selected_backend_integer
            .largest_integer_coefficient_bit_bound
            .max(largest_output_integer_coefficient_bit_bound);
        check_residual_affine_limit(
            "integer coefficient bits",
            largest_integer_coefficient_bit_bound,
            limits.max_integer_coefficient_bits,
        )?;
        if backend == ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion {
            // Atom replacement, normalization, expansion, validation and
            // conversion copy packed integer payloads while traversing the
            // structurally admitted tree. Scale the complete selected-path
            // visit census by the collision-grown final width so those
            // backend-internal byte copies cannot bypass integer-work
            // admission. Exponents are serialized as Num nodes too, hence
            // the fixed u16 width floor.
            let packed_integer_width = largest_integer_coefficient_bit_bound
                .max(u16::BITS as usize)
                .max(1);
            let atom_payload_bit_work = residual_affine_checked_mul(
                "Symbolica expression Atom payload bit work",
                addition_term_visit_bound,
                packed_integer_width,
            )?;
            selected_backend_integer.integer_bit_work_bound = residual_affine_checked_add(
                "Symbolica expression integer bit work",
                selected_backend_integer.integer_bit_work_bound,
                atom_payload_bit_work,
            )?;
        }
        // Every expanded contribution can participate in a maximal collision.
        // Charge the complete post-collection bit allowance to every one of C
        // contributions. This is conservative but, unlike a per-source-term
        // charge, cannot omit the global ceil(log2(C)) collision growth.
        let output_integer_bit_work_bound = residual_affine_checked_mul(
            "integer bit work",
            expanded_contribution_bound,
            largest_output_integer_coefficient_bit_bound,
        )?;
        let mut integer_bit_work_bound = residual_affine_checked_add(
            "integer bit work",
            selected_backend_integer.integer_bit_work_bound,
            output_integer_bit_work_bound,
        )?;
        if backend == ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion {
            // The expanded-Atom fast converter parses every final term and
            // reconstructs its integer coefficient through two additional
            // magnitude operations. This is separate from the C*B global
            // collision/collection charge above.
            integer_bit_work_bound = residual_affine_checked_add(
                "integer bit work",
                integer_bit_work_bound,
                residual_affine_checked_mul(
                    "Symbolica expression conversion integer bit work",
                    output_integer_bit_work_bound,
                    2,
                )?,
            )?;
        }
        check_residual_affine_limit(
            "integer bit work",
            integer_bit_work_bound,
            limits.max_integer_bit_work,
        )?;

        Ok(ResidualUnitAffinePolynomialPreflight {
            stats: ResidualUnitAffinePolynomialCompositionStats {
                source_terms,
                source_exponent_entries,
                expanded_contribution_bound,
                output_terms: 0,
                output_exponent_entry_bound: prospective_output_exponents,
                output_exponent_entries: 0,
                power_calls,
                native_power_heap_pair_bound,
                multiplication_term_pair_bound,
                addition_term_visit_bound,
                largest_kronecker_exponent_bits,
                largest_integer_coefficient_bit_bound,
                // Compatibility field name: this is the complete
                // pre-output integer-work census for whichever Symbolica
                // backend preflight selected.
                native_integer_bit_work_bound: selected_backend_integer.integer_bit_work_bound,
                integer_bit_work_bound,
            },
            backend,
        })
    }

    fn execute_residual_affine_polynomial_core(
        &self,
        source: &ParametricPolynomial,
        plan: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
        preflight: ResidualUnitAffinePolynomialPreflight,
    ) -> Result<ResidualAffinePolynomialComposition, ResidualUnitAffineCompositionError> {
        self.execute_residual_unit_affine_polynomial_raw(&source.raw, plan, limits, preflight)
    }

    fn execute_residual_unit_affine_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        plan: &ResidualAffineCompositionCorePlan,
        limits: ResidualUnitAffinePolynomialCompositionLimits,
        preflight: ResidualUnitAffinePolynomialPreflight,
    ) -> Result<ResidualAffinePolynomialComposition, ResidualUnitAffineCompositionError> {
        let mapped = match preflight.backend {
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator => {
                let ring = PolynomialRing::<IntegerRing, u16>::from_poly(&self.template.numerator);
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    source.evaluate_with_coeff_map(
                        |integer| self.template.numerator.constant(integer.clone()),
                        &plan.full_images,
                        &ring,
                    )
                }))
                .map_err(|_| {
                    ResidualUnitAffineCompositionError::SymbolicaPanic {
                        stage: "unit-affine polynomial composition",
                    }
                })?
            }
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion => {
                let mut mapped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let base_count = self.base.variables().len();
                    // Feed a support-filtered lazy iterator into Symbolica.
                    // Its replacement engine performs the collection
                    // internally, so RustRed owns no transient replacement
                    // Vec and does not build RHS Atoms absent from `source`.
                    let replacements = plan.nonfree_positions.iter().filter_map(|&position| {
                        let variable = base_count
                            .checked_add(position)
                            .expect("validated affine variable offset");
                        source
                            .exponents_iter()
                            .any(|exponents| exponents[variable] != 0)
                            .then(|| {
                                Replacement::new(
                                    self.variables[variable].to_atom(),
                                    plan.full_images[variable].to_expression(),
                                )
                            })
                    });
                    let expanded = source
                        .to_expression()
                        .replace_multiple(replacements)
                        .expand();
                    // Pin conversion to Symbolica's expanded-polynomial fast
                    // path. If a future backend revision changes `expand`
                    // semantics, fail at this typed boundary instead of
                    // allowing the converter's recursive fallback to re-enter
                    // polynomial `pow` and its u32 mixed-radix stride.
                    if !expanded.is_expanded::<Atom>(None)
                        || expanded.is_polynomial(false, false).is_none()
                    {
                        return Err(
                            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                                resource: "Symbolica expanded affine polynomial form",
                                actual: 0,
                                bound: 1,
                            },
                        );
                    }
                    expanded
                        .try_to_polynomial::<_, u16>(&Z, Some(self.variables.clone()))
                        .map_err(|_| {
                            ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                                resource: "Symbolica affine expression polynomial conversion",
                                actual: 1,
                                bound: 0,
                            }
                        })
                }))
                .map_err(|_| {
                    ResidualUnitAffineCompositionError::SymbolicaPanic {
                        stage: "unit-affine Symbolica expression composition",
                    }
                })??;
                // `try_to_polynomial` reconstructs an equal ordered variable
                // map behind a fresh Arc and permits discovering variables
                // while converting. Authenticate that no variable was added
                // or reordered, then restore the context's canonical Arc for
                // the common postvalidation boundary below.
                if mapped.variables.as_ref() != self.variables.as_ref() {
                    return Err(ResidualUnitAffineCompositionError::WrongContext);
                }
                mapped.variables = self.variables.clone();
                mapped
            }
        };

        if !Arc::ptr_eq(&mapped.variables, &self.variables) {
            return Err(ResidualUnitAffineCompositionError::WrongContext);
        }
        if mapped.nterms() > preflight.stats.expanded_contribution_bound {
            return Err(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "retained output terms",
                    actual: mapped.nterms(),
                    bound: preflight.stats.expanded_contribution_bound,
                },
            );
        }
        check_residual_affine_limit(
            "retained output terms",
            mapped.nterms(),
            limits.max_output_terms,
        )?;
        let output_exponent_entries = residual_affine_checked_mul(
            "retained output exponent entries",
            mapped.nterms(),
            self.variables.len(),
        )?;
        if output_exponent_entries > preflight.stats.output_exponent_entry_bound {
            return Err(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "retained output exponent entries",
                    actual: output_exponent_entries,
                    bound: preflight.stats.output_exponent_entry_bound,
                },
            );
        }
        check_residual_affine_limit(
            "retained output exponent entries",
            output_exponent_entries,
            limits.max_output_exponent_entries,
        )?;
        validate_polynomial_on_map(
            &mapped,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )
        .map_err(ParametricCoefficientError::from)?;

        let base_count = self.base.variables().len();
        for exponents in mapped.exponents_iter() {
            for &position in &plan.nonfree_positions {
                if exponents[base_count + position] != 0 {
                    return Err(ResidualUnitAffineCompositionError::NonFreeIndexSurvived {
                        position,
                    });
                }
            }
        }
        for coefficient in &mapped.coefficients {
            let actual_bits = residual_affine_integer_bits(coefficient)?;
            if actual_bits > preflight.stats.largest_integer_coefficient_bit_bound {
                return Err(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "retained integer coefficient bits",
                        actual: actual_bits,
                        bound: preflight.stats.largest_integer_coefficient_bit_bound,
                    },
                );
            }
            check_residual_affine_limit(
                "retained integer coefficient bits",
                actual_bits,
                limits.max_integer_coefficient_bits,
            )?;
        }

        let mut stats = preflight.stats;
        stats.output_terms = mapped.nterms();
        stats.output_exponent_entries = output_exponent_entries;
        Ok(ResidualAffinePolynomialComposition {
            value: ParametricPolynomial {
                raw: mapped,
                context: self.fingerprint.clone(),
            },
            stats,
        })
    }

    fn validate(&self, value: &ParametricCoefficient) -> Result<(), ParametricCoefficientError> {
        self.validate_with_limits(value, ExactAlgebraLimits::default())
    }

    pub fn validate_with_limits(
        &self,
        value: &ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<(), ParametricCoefficientError> {
        if value.context.as_ref() != self.fingerprint.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        validate_coefficient_on_map(&value.raw, &self.variables, limits)?;
        Ok(())
    }

    /// Preflight and validate one coefficient under caller-supplied remaining
    /// aggregate row allowances.  Shape bounds are checked before exact map
    /// validation; arbitrary-precision coefficient magnitudes are accumulated
    /// transactionally before the backend validator is entered.
    pub(crate) fn preflight_validation_payload_with_limits(
        &self,
        value: &ParametricCoefficient,
        exact_limits: ExactAlgebraLimits,
        max_source_terms: usize,
        max_source_exponent_entries: usize,
        max_source_integer_bits: usize,
    ) -> Result<ParametricCoefficientValidationPayloadCensus, ParametricCoefficientError> {
        if value.context.as_ref() != self.fingerprint.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        let source_terms = value
            .raw
            .numerator
            .nterms()
            .checked_add(value.raw.denominator.nterms())
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric coefficient validation source terms",
            })?;
        check_limit(
            "parametric coefficient validation source terms",
            source_terms,
            max_source_terms,
        )?;
        let source_exponent_entries = value
            .raw
            .numerator
            .exponents
            .len()
            .checked_add(value.raw.denominator.exponents.len())
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric coefficient validation source exponent entries",
            })?;
        check_limit(
            "parametric coefficient validation source exponent entries",
            source_exponent_entries,
            max_source_exponent_entries,
        )?;
        let mut source_integer_bits = 0usize;
        for coefficient in value
            .raw
            .numerator
            .coefficients
            .iter()
            .chain(value.raw.denominator.coefficients.iter())
        {
            let bits = usize::try_from(integer_magnitude_bits(coefficient)).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric coefficient validation source integer bits",
                }
            })?;
            let prospective = source_integer_bits.checked_add(bits).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric coefficient validation source integer bits",
                },
            )?;
            check_limit(
                "parametric coefficient validation source integer bits",
                prospective,
                max_source_integer_bits,
            )?;
            source_integer_bits = prospective;
        }
        validate_coefficient_on_map(&value.raw, &self.variables, exact_limits)?;
        Ok(ParametricCoefficientValidationPayloadCensus {
            source_terms,
            source_exponent_entries,
            source_integer_bits,
        })
    }

    pub(crate) fn preflight_polynomial_validation_payload_with_limits(
        &self,
        value: &ParametricPolynomial,
        exact_limits: ExactAlgebraLimits,
        max_source_terms: usize,
        max_source_exponent_entries: usize,
        max_source_integer_bits: usize,
    ) -> Result<ParametricPolynomialValidationPayloadCensus, ParametricCoefficientError> {
        if value.context.as_ref() != self.fingerprint.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        let source_terms = value.raw.nterms();
        check_limit(
            "parametric polynomial validation source terms",
            source_terms,
            max_source_terms,
        )?;
        let source_exponent_entries = value.raw.exponents.len();
        check_limit(
            "parametric polynomial validation source exponent entries",
            source_exponent_entries,
            max_source_exponent_entries,
        )?;
        let mut source_integer_bits = 0usize;
        for coefficient in &value.raw.coefficients {
            let bits = usize::try_from(integer_magnitude_bits(coefficient)).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric polynomial validation source integer bits",
                }
            })?;
            let prospective = source_integer_bits.checked_add(bits).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric polynomial validation source integer bits",
                },
            )?;
            check_limit(
                "parametric polynomial validation source integer bits",
                prospective,
                max_source_integer_bits,
            )?;
            source_integer_bits = prospective;
        }
        validate_polynomial_on_map(
            &value.raw,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            exact_limits,
        )?;
        Ok(ParametricPolynomialValidationPayloadCensus {
            source_terms,
            source_exponent_entries,
            source_integer_bits,
        })
    }

    fn validate_parametric_polynomial(
        &self,
        value: &ParametricPolynomial,
    ) -> Result<(), ParametricCoefficientError> {
        self.validate_polynomial_with_limits(value, ExactAlgebraLimits::default())
    }

    fn validate_shift(&self, shift: &[i64]) -> Result<(), ParametricCoefficientError> {
        if shift.len() == self.index_count() {
            Ok(())
        } else {
            Err(ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: shift.len(),
            })
        }
    }

    fn validate_exact_shift(&self, shift: &[Integer]) -> Result<(), ParametricCoefficientError> {
        if shift.len() == self.index_count() {
            Ok(())
        } else {
            Err(ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: shift.len(),
            })
        }
    }

    fn validate_index_permutation(
        &self,
        source_to_target: &[usize],
    ) -> Result<(), ParametricCoefficientError> {
        if source_to_target.len() != self.index_count() {
            return Err(ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: source_to_target.len(),
            });
        }
        for (source, &target) in source_to_target.iter().enumerate() {
            if target >= self.index_count() {
                return Err(ParametricCoefficientError::InvalidIndexPermutation);
            }
            if source_to_target[..source].contains(&target) {
                return Err(ParametricCoefficientError::InvalidIndexPermutation);
            }
        }
        Ok(())
    }

    fn validate_partial_assignment(
        &self,
        assignment: &PartialIndexAssignment,
    ) -> Result<(), ParametricCoefficientError> {
        if assignment.arity == self.index_count() {
            Ok(())
        } else {
            Err(ParametricCoefficientError::WrongIndexArity {
                expected: self.index_count(),
                actual: assignment.arity,
            })
        }
    }

    fn raw_uses_extended_map(&self, raw: &Coefficient) -> bool {
        validate_coefficient_on_map(raw, &self.variables, ExactAlgebraLimits::default()).is_ok()
    }

    fn wrap_unchecked(&self, raw: Coefficient) -> ParametricCoefficient {
        debug_assert!(self.raw_uses_extended_map(&raw));
        ParametricCoefficient {
            raw,
            context: self.fingerprint.clone(),
        }
    }

    fn wrap_checked(
        &self,
        raw: Coefficient,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.wrap_checked_with_limits(raw, ExactAlgebraLimits::default())
    }

    fn wrap_checked_with_limits(
        &self,
        raw: Coefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        validate_coefficient_on_map(&raw, &self.variables, limits)?;
        Ok(self.wrap_unchecked(raw))
    }

    /// Canonicalize a valid fraction by a polynomial gcd under the caller's
    /// exact-work budget.  Symbolica's raw division assumes normalized
    /// operands and can otherwise leave an internal factor such as `n/n`.
    fn normalize_with_limits(
        &self,
        value: ParametricCoefficient,
        limits: ExactAlgebraLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        self.validate_with_limits(&value, limits)?;
        let operations = value
            .raw
            .numerator
            .nterms()
            .checked_mul(value.raw.denominator.nterms())
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "guarded division normalization term pairs",
            })?;
        check_limit(
            "guarded division normalization term pairs",
            operations,
            limits.max_term_operations,
        )?;
        let numerator = value.raw.numerator;
        let denominator = value.raw.denominator;
        let normalized = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            <Coefficient as FromNumeratorAndDenominator<IntegerRing, IntegerRing, u16>>::from_num_den(
                numerator,
                denominator,
                &Z,
                true,
            )
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while normalizing a checked guarded division".to_owned(),
            )
        })?;
        self.wrap_checked_with_limits(normalized, limits)
    }

    fn extend_base_polynomial(
        &self,
        source: &CoefficientPolynomial,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        validate_polynomial_on_map(
            source,
            self.base.variables(),
            crate::CoefficientPolynomialPart::Numerator,
            ExactAlgebraLimits::default(),
        )?;
        let mut result = self
            .template
            .numerator
            .zero_with_capacity(source.coefficients.len());
        let mut exponents = vec![0_u16; self.variables.len()];
        for (coefficient, source_exponents) in
            source.coefficients.iter().zip(source.exponents_iter())
        {
            exponents.fill(0);
            exponents[..self.base.variables().len()].copy_from_slice(source_exponents);
            result.append_monomial(coefficient.clone(), &exponents);
        }
        Ok(result)
    }

    fn translate_polynomial_raw<T: ParametricTranslationComponent>(
        &self,
        source: &CoefficientPolynomial,
        shift: &[T],
        limits: ParametricArithmeticLimits,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let preflight = self.preflight_translate_polynomial_raw(source, shift, limits)?;
        self.execute_translate_polynomial_raw(source, shift, limits, preflight)
    }

    fn preflight_translate_polynomial_raw<T: ParametricTranslationComponent>(
        &self,
        source: &CoefficientPolynomial,
        shift: &[T],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialTranslationPreflight, ParametricCoefficientError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        check_limit(
            "parametric translation source terms",
            source.nterms(),
            limits.max_source_terms,
        )?;

        let base_count = self.base.variables().len();
        let mut output_term_bound = 0_usize;
        let mut power_operation_bound = 0_usize;
        let mut largest_contribution_bits = 0usize;
        let mut integer_bit_work_bound = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let mut term_bound = 1_usize;
            for (position, offset) in shift.iter().enumerate() {
                if offset.is_numeric_zero() {
                    continue;
                }
                let exponent = usize::from(exponents[base_count + position]);
                if exponent != 0 {
                    power_operation_bound = checked_parametric_add(
                        "parametric translation power operations",
                        power_operation_bound,
                        term_bound,
                    )?;
                }
                term_bound = checked_parametric_mul(
                    "parametric translation output terms",
                    term_bound,
                    exponent + 1,
                )?;
            }
            output_term_bound = checked_parametric_add(
                "parametric translation output terms",
                output_term_bound,
                term_bound,
            )?;
            let mut requested = integer_magnitude_bits(coefficient);
            for (position, offset) in shift.iter().enumerate() {
                if offset.is_numeric_zero() {
                    continue;
                }
                let exponent = u128::from(exponents[base_count + position]);
                if exponent == 0 {
                    continue;
                }
                requested = requested.checked_add(exponent).ok_or(
                    ParametricCoefficientError::ResourceCountOverflow {
                        resource: "parametric translation integer bits",
                    },
                )?;
                let offset_bits = offset.magnitude_bits();
                if offset_bits > 1 {
                    requested = requested
                        .checked_add(offset_bits.checked_mul(exponent).ok_or(
                            ParametricCoefficientError::ResourceCountOverflow {
                                resource: "parametric translation integer bits",
                            },
                        )?)
                        .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                            resource: "parametric translation integer bits",
                        })?;
                }
            }
            let requested = usize::try_from(requested).map_err(|_| {
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric translation integer bits",
                }
            })?;
            check_limit(
                "parametric translation integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_contribution_bits = largest_contribution_bits.max(requested);
            integer_bit_work_bound = checked_parametric_add(
                "parametric translation integer-bit work",
                integer_bit_work_bound,
                checked_parametric_mul(
                    "parametric translation integer-bit work",
                    term_bound,
                    requested,
                )?,
            )?;
        }
        check_limit(
            "parametric translation output terms",
            output_term_bound,
            limits
                .max_output_terms
                .min(limits.exact_algebra.max_polynomial_terms),
        )?;
        check_limit(
            "parametric translation power operations",
            power_operation_bound,
            limits.max_specialization_power_operations,
        )?;

        // Expanding (n+a)^e produces coefficients containing binomial(e,k)
        // and powers of `a`. For each contribution use binomial(e,k) <= 2^e,
        // then charge ceil(log2(output_term_bound)) for worst-case collection
        // of equal monomials.
        let collision_bits = parametric_ceil_log2(output_term_bound);
        let collected_bits = largest_contribution_bits
            .checked_add(collision_bits)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "parametric translation integer bits",
            })?;
        check_limit(
            "parametric translation integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;
        integer_bit_work_bound = checked_parametric_add(
            "parametric translation integer-bit work",
            integer_bit_work_bound,
            checked_parametric_mul(
                "parametric translation integer-bit work",
                output_term_bound,
                collision_bits,
            )?,
        )?;
        let output_exponent_entry_bound = checked_parametric_mul(
            "parametric translation output exponent entries",
            output_term_bound,
            self.variables.len(),
        )?;
        let largest_output_integer_capacity_byte_bound = integer_limb_payload_byte_bound(
            collected_bits,
            "parametric translation retained output bytes",
        )?
        .max(largest_integer_owned_capacity_bytes(source)?);
        let output_coefficient_capacity_bound = parametric_vec_capacity_bound(
            output_term_bound,
            "parametric translation retained output bytes",
        )?
        .max(source.coefficients.capacity());
        let output_exponent_capacity_bound = parametric_vec_capacity_bound(
            output_exponent_entry_bound,
            "parametric translation retained output bytes",
        )?
        .max(source.exponents.capacity());
        let retained_output_byte_bound = authenticated_polynomial_retained_byte_envelope(
            size_of::<ParametricPolynomial>(),
            output_term_bound,
            output_exponent_entry_bound,
            collected_bits,
            output_coefficient_capacity_bound,
            output_exponent_capacity_bound,
            largest_output_integer_capacity_byte_bound,
            "parametric translation retained output bytes",
        )?;
        Ok(ParametricPolynomialTranslationPreflight {
            source_terms: source.nterms(),
            source_exponent_entries: source.exponents.len(),
            output_term_bound,
            output_exponent_entry_bound,
            power_operation_bound,
            largest_output_integer_bit_bound: collected_bits,
            largest_output_integer_capacity_byte_bound,
            output_coefficient_capacity_bound,
            output_exponent_capacity_bound,
            integer_bit_work_bound,
            retained_output_term_bound: output_term_bound,
            retained_output_byte_bound,
        })
    }

    fn execute_translate_polynomial_raw<T: ParametricTranslationComponent>(
        &self,
        source: &CoefficientPolynomial,
        shift: &[T],
        limits: ParametricArithmeticLimits,
        preflight: ParametricPolynomialTranslationPreflight,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let mut result = source.clone();
        let base_count = self.base.variables().len();
        for (position, offset) in shift.iter().enumerate() {
            if offset.is_numeric_zero() {
                continue;
            }
            let variable_position = base_count + position;
            if !source
                .exponents_iter()
                .any(|exponents| exponents[variable_position] != 0)
            {
                // The preflight correctly charges no offset bits when this
                // index is absent.  Do not canonicalize or clone a possibly
                // huge irrelevant GMP component during execution.
                continue;
            }
            let variable = self
                .template
                .numerator
                .variable(&self.index_variables[position])
                .map_err(ParametricCoefficientError::Symbolica)?;
            result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let replacement = &variable
                    + &self
                        .template
                        .numerator
                        .constant(offset.to_canonical_integer());
                result.replace_with_poly(variable_position, &replacement)
            }))
            .map_err(|_| {
                ParametricCoefficientError::Symbolica(
                    "Symbolica panicked during checked parametric translation".to_owned(),
                )
            })?;
        }
        if result.variables.as_ref() != self.variables.as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "parametric translation",
        )?;
        let retained_bytes = polynomial_retained_bytes_with_wrapper(
            &result,
            size_of::<ParametricPolynomial>(),
            "parametric translation retained output bytes",
        )?;
        if retained_bytes > preflight.retained_output_byte_bound {
            return Err(ParametricCoefficientError::Symbolica(format!(
                "parametric translation escaped its retained-byte preflight envelope: actual {retained_bytes}, bound {}, source terms {}, shift arity {}, terms {}, coefficient capacity {}, exponent capacity {}, planned terms {}, planned coefficient capacity {}, planned exponent capacity {}",
                preflight.retained_output_byte_bound,
                source.nterms(),
                shift.len(),
                result.nterms(),
                result.coefficients.capacity(),
                result.exponents.capacity(),
                preflight.output_term_bound,
                preflight.output_coefficient_capacity_bound,
                preflight.output_exponent_capacity_bound,
            )));
        }
        validate_polynomial_on_map(
            &result,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }

    fn permute_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        source_to_target: &[usize],
        limits: ParametricArithmeticLimits,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        self.validate_index_permutation(source_to_target)?;
        check_limit(
            "parametric permutation source terms",
            source.nterms(),
            limits.max_source_terms,
        )?;
        check_limit(
            "parametric permutation output terms",
            source.nterms(),
            limits.max_output_terms,
        )?;
        let base_count = self.base.variables().len();
        let mut result = self.template.numerator.zero_with_capacity(source.nterms());
        let mut target_exponents = vec![0_u16; self.variables.len()];
        for (coefficient, source_exponents) in
            source.coefficients.iter().zip(source.exponents_iter())
        {
            target_exponents.fill(0);
            target_exponents[..base_count].copy_from_slice(&source_exponents[..base_count]);
            for (source_index, &target_index) in source_to_target.iter().enumerate() {
                target_exponents[base_count + target_index] =
                    source_exponents[base_count + source_index];
            }
            result.append_monomial(coefficient.clone(), &target_exponents);
        }
        validate_polynomial_on_map(
            &result,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }

    fn partially_specialize_polynomial_raw_checked(
        &self,
        source: &CoefficientPolynomial,
        assignment: &PartialIndexAssignment,
        limits: ParametricArithmeticLimits,
    ) -> Result<
        (CoefficientPolynomial, PartialPolynomialSpecializationStats),
        ParametricCoefficientError,
    > {
        self.validate_partial_assignment(assignment)?;
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        check_limit(
            "partial polynomial specialization source terms",
            source.nterms(),
            limits.max_source_terms,
        )?;
        let power_operations = source
            .nterms()
            .checked_mul(assignment.entries.len())
            .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                resource: "partial polynomial specialization power operations",
            })?;
        check_limit(
            "partial polynomial specialization power operations",
            power_operations,
            limits.max_specialization_power_operations,
        )?;
        // Integer substitution only removes exponents, so at most all source
        // monomials survive before exact collection.
        check_limit(
            "partial polynomial specialization output terms",
            source.nterms(),
            limits.max_output_terms,
        )?;

        let base_count = self.base.variables().len();
        let mut largest_term_bits = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let requested = partial_specialization_integer_bit_bound(
                coefficient,
                exponents,
                base_count,
                assignment.entries(),
            )?;
            check_limit(
                "partial polynomial specialization integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_term_bits = largest_term_bits.max(requested);
        }
        let collision_bits = if source.nterms() <= 1 {
            0
        } else {
            usize::BITS as usize - (source.nterms() - 1).leading_zeros() as usize
        };
        let integer_bit_bound = largest_term_bits.checked_add(collision_bits).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "partial polynomial specialization integer bits",
            },
        )?;
        check_limit(
            "partial polynomial specialization integer bits",
            integer_bit_bound,
            limits.max_specialization_integer_bits,
        )?;

        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut result = source.clone();
            for &(position, value) in assignment.entries() {
                result = result.replace(base_count + position, &Integer::from(value));
            }
            result
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked during checked partial polynomial specialization".to_owned(),
            )
        })?;
        check_limit(
            "partial polynomial specialization output terms",
            raw.nterms(),
            limits.max_output_terms,
        )?;
        validate_polynomial_on_map(
            &raw,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        let output_terms = raw.nterms();
        Ok((
            raw,
            PartialPolynomialSpecializationStats {
                source_terms: source.nterms(),
                output_terms,
                power_operations,
                integer_bit_bound,
            },
        ))
    }

    fn specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let preflight = self.preflight_specialize_polynomial_raw(source, assignment, limits)?;
        self.execute_specialize_polynomial_raw(source, assignment, limits, preflight)
    }

    fn preflight_specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricPolynomialSpecializationPreflight, ParametricCoefficientError> {
        validate_polynomial_on_map(
            source,
            &self.variables,
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        check_limit(
            "coefficient specialization source terms",
            source.nterms(),
            limits.max_source_terms,
        )?;
        check_limit(
            "coefficient specialization output terms",
            source.nterms(),
            limits
                .max_output_terms
                .min(limits.exact_algebra.max_polynomial_terms),
        )?;
        let operations = source.nterms().checked_mul(self.index_count()).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "coefficient specialization power operations",
            },
        )?;
        check_limit(
            "coefficient specialization power operations",
            operations,
            limits.max_specialization_power_operations,
        )?;

        let base_count = self.base.variables().len();
        // Preflight every arbitrary-precision power before constructing any
        // output coefficient.  Counting calls alone is insufficient:
        // `value^exponent` can allocate an integer linear in `exponent` bits
        // even when the source polynomial contains only one term.
        let mut largest_term_bits = 0usize;
        let mut integer_bit_work_bound = 0usize;
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let requested =
                specialization_integer_bit_bound(coefficient, exponents, base_count, assignment)?;
            check_limit(
                "coefficient specialization integer bits",
                requested,
                limits.max_specialization_integer_bits,
            )?;
            largest_term_bits = largest_term_bits.max(requested);
            integer_bit_work_bound = checked_parametric_add(
                "coefficient specialization integer-bit work",
                integer_bit_work_bound,
                requested,
            )?;
        }
        let collision_bits = parametric_ceil_log2(source.nterms());
        let collected_bits = largest_term_bits.checked_add(collision_bits).ok_or(
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "coefficient specialization integer bits",
            },
        )?;
        check_limit(
            "coefficient specialization integer bits",
            collected_bits,
            limits.max_specialization_integer_bits,
        )?;
        integer_bit_work_bound = checked_parametric_add(
            "coefficient specialization integer-bit work",
            integer_bit_work_bound,
            checked_parametric_mul(
                "coefficient specialization integer-bit work",
                source.nterms(),
                collision_bits,
            )?,
        )?;
        let output_exponent_entry_bound = checked_parametric_mul(
            "coefficient specialization output exponent entries",
            source.nterms(),
            base_count,
        )?;
        let largest_output_integer_capacity_byte_bound = integer_limb_payload_byte_bound(
            collected_bits,
            "coefficient specialization retained output bytes",
        )?
        .max(largest_integer_owned_capacity_bytes(source)?);
        let output_coefficient_capacity_bound = source.nterms();
        let output_exponent_capacity_bound = output_exponent_entry_bound;
        let retained_output_byte_bound = authenticated_polynomial_retained_byte_envelope(
            size_of::<BasePolynomial>(),
            output_coefficient_capacity_bound,
            output_exponent_capacity_bound,
            collected_bits,
            source.nterms(),
            output_exponent_entry_bound,
            largest_output_integer_capacity_byte_bound,
            "coefficient specialization retained output bytes",
        )?;
        Ok(ParametricPolynomialSpecializationPreflight {
            source_terms: source.nterms(),
            source_exponent_entries: source.exponents.len(),
            output_term_bound: source.nterms(),
            output_exponent_entry_bound,
            power_operation_bound: operations,
            largest_output_integer_bit_bound: collected_bits,
            largest_output_integer_capacity_byte_bound,
            output_coefficient_capacity_bound,
            output_exponent_capacity_bound,
            integer_bit_work_bound,
            retained_output_term_bound: source.nterms(),
            retained_output_byte_bound,
        })
    }

    fn execute_specialize_polynomial_raw(
        &self,
        source: &CoefficientPolynomial,
        assignment: &[i64],
        limits: ParametricArithmeticLimits,
        preflight: ParametricPolynomialSpecializationPreflight,
    ) -> Result<CoefficientPolynomial, ParametricCoefficientError> {
        let base_count = self.base.variables().len();
        let mut result = self
            .base
            .template()
            .numerator
            .zero_with_capacity(source.nterms());
        for (coefficient, exponents) in source.coefficients.iter().zip(source.exponents_iter()) {
            let mut specialized = coefficient.clone();
            for (position, value) in assignment.iter().copied().enumerate() {
                let exponent = exponents[base_count + position];
                if exponent != 0 {
                    specialized = specialized * Integer::from(value).pow(u64::from(exponent));
                }
            }
            result.append_monomial(specialized, &exponents[..base_count]);
        }
        if result.variables.as_ref() != self.base.variables().as_ref() {
            return Err(ParametricCoefficientError::WrongContext);
        }
        verify_polynomial_execution_envelope(
            &result,
            preflight.output_term_bound,
            preflight.output_exponent_entry_bound,
            preflight.largest_output_integer_bit_bound,
            "coefficient specialization",
        )?;
        let retained_bytes = polynomial_retained_bytes_with_wrapper(
            &result,
            size_of::<BasePolynomial>(),
            "coefficient specialization retained output bytes",
        )?;
        if retained_bytes > preflight.retained_output_byte_bound {
            return Err(ParametricCoefficientError::Symbolica(
                "coefficient specialization escaped its retained-byte preflight envelope"
                    .to_owned(),
            ));
        }
        validate_polynomial_on_map(
            &result,
            self.base.variables(),
            crate::CoefficientPolynomialPart::Numerator,
            limits.exact_algebra,
        )?;
        Ok(result)
    }
}

#[derive(Clone, Copy, Debug)]
struct NormalizedRationalRetainedEnvelope {
    term_bound: usize,
    byte_bound: usize,
    integer_bit_payload_bound: usize,
}

fn coefficient_translation_preflight(
    _numerator_source: &CoefficientPolynomial,
    _denominator_source: &CoefficientPolynomial,
    numerator: ParametricPolynomialTranslationPreflight,
    denominator: ParametricPolynomialTranslationPreflight,
    numerator_is_zero: bool,
    denominator_is_one: bool,
    variable_count: usize,
    limits: ParametricArithmeticLimits,
) -> Result<ParametricCoefficientTranslationPreflight, ParametricCoefficientError> {
    // Canonical inputs are coprime and integral translation preserves that
    // property. `from_num_den(..., false)` below only normalizes denominator
    // sign, so there is no GCD input-pair work to charge here.
    let normalization_input_term_pair_bound = 0;
    let numerator_factor_terms = numerator.output_term_bound;
    let numerator_factor_bits = numerator.largest_output_integer_bit_bound;
    let denominator_factor_terms = denominator.output_term_bound;
    let denominator_factor_bits = denominator.largest_output_integer_bit_bound;
    check_limit(
        "parametric translation normalized integer bits",
        numerator_factor_bits.max(denominator_factor_bits),
        limits.max_specialization_integer_bits,
    )?;
    let normalized = normalized_rational_retained_envelope(
        numerator.output_term_bound,
        numerator.largest_output_integer_bit_bound,
        numerator.largest_output_integer_capacity_byte_bound,
        numerator.output_coefficient_capacity_bound,
        numerator.output_exponent_capacity_bound,
        numerator_factor_terms,
        numerator_factor_bits,
        denominator.output_term_bound,
        denominator.largest_output_integer_bit_bound,
        denominator.largest_output_integer_capacity_byte_bound,
        denominator.output_coefficient_capacity_bound,
        denominator.output_exponent_capacity_bound,
        denominator_factor_terms,
        denominator_factor_bits,
        numerator_is_zero,
        denominator_is_one,
        variable_count,
        size_of::<ParametricCoefficient>(),
        "parametric translation normalized coefficient",
    )?;
    let mapped_integer_work = checked_parametric_add(
        "parametric translation integer-bit work",
        numerator.integer_bit_work_bound,
        denominator.integer_bit_work_bound,
    )?;
    Ok(ParametricCoefficientTranslationPreflight {
        numerator,
        denominator,
        source_terms: checked_parametric_add(
            "parametric translation source terms",
            numerator.source_terms,
            denominator.source_terms,
        )?,
        output_term_bound: checked_parametric_add(
            "parametric translation output terms",
            numerator.output_term_bound,
            denominator.output_term_bound,
        )?,
        power_operation_bound: checked_parametric_add(
            "parametric translation power operations",
            numerator.power_operation_bound,
            denominator.power_operation_bound,
        )?,
        integer_bit_work_bound: checked_parametric_add(
            "parametric translation integer-bit work",
            mapped_integer_work,
            normalized.integer_bit_payload_bound,
        )?,
        normalization_input_term_pair_bound,
        normalized_coefficient_term_bound: normalized.term_bound,
        normalized_coefficient_byte_bound: normalized.byte_bound,
    })
}

fn coefficient_specialization_preflight(
    numerator_source: &CoefficientPolynomial,
    denominator_source: &CoefficientPolynomial,
    numerator: ParametricPolynomialSpecializationPreflight,
    denominator: ParametricPolynomialSpecializationPreflight,
    numerator_is_zero: bool,
    denominator_is_one: bool,
    variable_count: usize,
    limits: ParametricArithmeticLimits,
) -> Result<ParametricCoefficientSpecializationPreflight, ParametricCoefficientError> {
    let normalization_input_term_pair_bound = checked_parametric_mul(
        "coefficient specialization normalization input term pairs",
        numerator.output_term_bound.max(1),
        denominator.output_term_bound,
    )?;
    check_limit(
        "coefficient specialization normalization input term pairs",
        normalization_input_term_pair_bound,
        limits.exact_algebra.max_term_operations,
    )?;
    let (
        numerator_factor_terms,
        numerator_factor_bits,
        denominator_factor_terms,
        denominator_factor_bits,
    ) = if numerator_is_zero || denominator_is_one {
        (
            numerator.output_term_bound,
            numerator.largest_output_integer_bit_bound,
            denominator.output_term_bound,
            denominator.largest_output_integer_bit_bound,
        )
    } else {
        let numerator_factor = normalized_factor_envelope_from_source(
            numerator_source,
            0,
            variable_count,
            numerator.output_term_bound,
            numerator.largest_output_integer_bit_bound,
            limits
                .exact_algebra
                .max_polynomial_terms
                .min(limits.max_output_terms),
            "coefficient specialization normalized numerator support",
        )?;
        let denominator_factor = normalized_factor_envelope_from_source(
            denominator_source,
            0,
            variable_count,
            denominator.output_term_bound,
            denominator.largest_output_integer_bit_bound,
            limits
                .exact_algebra
                .max_polynomial_terms
                .min(limits.max_output_terms),
            "coefficient specialization normalized denominator support",
        )?;
        (
            numerator_factor.0,
            numerator_factor.1,
            denominator_factor.0,
            denominator_factor.1,
        )
    };
    check_limit(
        "coefficient specialization normalized integer bits",
        numerator_factor_bits.max(denominator_factor_bits),
        limits.max_specialization_integer_bits,
    )?;
    let normalized = normalized_rational_retained_envelope(
        numerator.output_term_bound,
        numerator.largest_output_integer_bit_bound,
        numerator.largest_output_integer_capacity_byte_bound,
        numerator.output_coefficient_capacity_bound,
        numerator.output_exponent_capacity_bound,
        numerator_factor_terms,
        numerator_factor_bits,
        denominator.output_term_bound,
        denominator.largest_output_integer_bit_bound,
        denominator.largest_output_integer_capacity_byte_bound,
        denominator.output_coefficient_capacity_bound,
        denominator.output_exponent_capacity_bound,
        denominator_factor_terms,
        denominator_factor_bits,
        numerator_is_zero,
        denominator_is_one,
        variable_count,
        size_of::<Coefficient>(),
        "coefficient specialization normalized coefficient",
    )?;
    let mapped_integer_work = checked_parametric_add(
        "coefficient specialization integer-bit work",
        numerator.integer_bit_work_bound,
        denominator.integer_bit_work_bound,
    )?;
    Ok(ParametricCoefficientSpecializationPreflight {
        numerator,
        denominator,
        source_terms: checked_parametric_add(
            "coefficient specialization source terms",
            numerator.source_terms,
            denominator.source_terms,
        )?,
        output_term_bound: checked_parametric_add(
            "coefficient specialization output terms",
            numerator.output_term_bound,
            denominator.output_term_bound,
        )?,
        power_operation_bound: checked_parametric_add(
            "coefficient specialization power operations",
            numerator.power_operation_bound,
            denominator.power_operation_bound,
        )?,
        integer_bit_work_bound: checked_parametric_add(
            "coefficient specialization integer-bit work",
            mapped_integer_work,
            normalized.integer_bit_payload_bound,
        )?,
        normalization_input_term_pair_bound,
        normalized_coefficient_term_bound: normalized.term_bound,
        normalized_coefficient_byte_bound: normalized.byte_bound,
        denominator_guard_term_bound: denominator.output_term_bound,
        denominator_guard_byte_bound: denominator.retained_output_byte_bound,
    })
}

#[allow(clippy::too_many_arguments)]
fn normalized_rational_retained_envelope(
    numerator_mapped_terms: usize,
    numerator_mapped_bits: usize,
    numerator_mapped_capacity_bytes: usize,
    numerator_mapped_coefficient_capacity: usize,
    numerator_mapped_exponent_capacity: usize,
    numerator_factor_terms: usize,
    numerator_factor_bits: usize,
    denominator_mapped_terms: usize,
    denominator_mapped_bits: usize,
    denominator_mapped_capacity_bytes: usize,
    denominator_mapped_coefficient_capacity: usize,
    denominator_mapped_exponent_capacity: usize,
    denominator_factor_terms: usize,
    denominator_factor_bits: usize,
    numerator_is_zero: bool,
    denominator_is_one: bool,
    variable_count: usize,
    wrapper_bytes: usize,
    resource: &'static str,
) -> Result<NormalizedRationalRetainedEnvelope, ParametricCoefficientError> {
    let (numerator_terms, numerator_bits, denominator_terms, denominator_bits) =
        if numerator_is_zero {
            (0, 0, 1, 1)
        } else if denominator_is_one {
            (
                numerator_mapped_terms,
                numerator_mapped_bits,
                denominator_mapped_terms,
                denominator_mapped_bits,
            )
        } else {
            (
                numerator_factor_terms,
                numerator_factor_bits,
                denominator_factor_terms,
                denominator_factor_bits,
            )
        };
    let numerator_exponents = checked_parametric_mul(resource, numerator_terms, variable_count)?;
    let denominator_exponents =
        checked_parametric_mul(resource, denominator_terms, variable_count)?;
    let numerator_payload = polynomial_sparse_payload_byte_envelope(
        numerator_terms,
        numerator_exponents,
        numerator_bits,
        numerator_mapped_coefficient_capacity,
        numerator_mapped_exponent_capacity,
        numerator_mapped_capacity_bytes,
        resource,
    )?;
    let denominator_payload = polynomial_sparse_payload_byte_envelope(
        denominator_terms,
        denominator_exponents,
        denominator_bits,
        denominator_mapped_coefficient_capacity,
        denominator_mapped_exponent_capacity,
        denominator_mapped_capacity_bytes,
        resource,
    )?;
    let numerator_integer_payload =
        checked_parametric_mul(resource, numerator_terms, numerator_bits)?;
    let denominator_integer_payload =
        checked_parametric_mul(resource, denominator_terms, denominator_bits)?;
    Ok(NormalizedRationalRetainedEnvelope {
        term_bound: checked_parametric_add(resource, numerator_terms, denominator_terms)?,
        byte_bound: checked_parametric_add(
            resource,
            wrapper_bytes,
            checked_parametric_add(resource, numerator_payload, denominator_payload)?,
        )?,
        integer_bit_payload_bound: checked_parametric_add(
            resource,
            numerator_integer_payload,
            denominator_integer_payload,
        )?,
    })
}

fn normalized_factor_envelope_from_source(
    source: &CoefficientPolynomial,
    first_variable: usize,
    variable_count: usize,
    mapped_term_bound: usize,
    mapped_integer_bit_bound: usize,
    successful_term_cap: usize,
    resource: &'static str,
) -> Result<(usize, usize), ParametricCoefficientError> {
    if source.is_zero() {
        return Ok((0, 0));
    }
    // A mixed-radix Kronecker image with radices degree_i+1 is injective on
    // every possible factor. Its degree is support_size-1. The univariate
    // Landau-Mignotte factor-height bound then gives
    //   bits(factor) <= bits(input) + degree + ceil(log2(input terms)).
    // This is intentionally coarse, but it remains finite, allocation-free,
    // and sound even when exact GCD division turns a sparse input into a dense
    // quotient such as (x^n-1)/(x-1).
    let mut support_size = 1usize;
    let variable_end = first_variable
        .checked_add(variable_count)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })?;
    if variable_end > source.variables.len() {
        return Err(ParametricCoefficientError::WrongContext);
    }
    for variable in first_variable..variable_end {
        let mut degree = 0usize;
        for exponents in source.exponents_iter() {
            degree = degree.max(usize::from(exponents[variable]));
        }
        support_size = checked_parametric_mul(resource, support_size, degree + 1)?;
    }
    // Exact division may materialize every monomial in this support before
    // the post-normalization authenticator sees the result. Reject the dense
    // support prospectively; `min(successful_term_cap)` would only turn the
    // configured term cap into a post-allocation publication gate.
    check_limit(resource, support_size, successful_term_cap)?;
    let term_bound = support_size;
    let integer_bit_bound = checked_parametric_add(
        resource,
        mapped_integer_bit_bound.max(1),
        checked_parametric_add(
            resource,
            support_size.saturating_sub(1),
            parametric_ceil_log2(mapped_term_bound),
        )?,
    )?;
    Ok((term_bound, integer_bit_bound))
}

fn authenticated_polynomial_retained_byte_envelope(
    wrapper_bytes: usize,
    terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    minimum_coefficient_capacity: usize,
    minimum_exponent_capacity: usize,
    minimum_per_integer_payload_bytes: usize,
    resource: &'static str,
) -> Result<usize, ParametricCoefficientError> {
    checked_parametric_add(
        resource,
        wrapper_bytes,
        polynomial_sparse_payload_byte_envelope(
            terms,
            exponent_entries,
            integer_bits,
            minimum_coefficient_capacity,
            minimum_exponent_capacity,
            minimum_per_integer_payload_bytes,
            resource,
        )?,
    )
}

fn polynomial_sparse_payload_byte_envelope(
    terms: usize,
    exponent_entries: usize,
    integer_bits: usize,
    minimum_coefficient_capacity: usize,
    minimum_exponent_capacity: usize,
    minimum_per_integer_payload_bytes: usize,
    resource: &'static str,
) -> Result<usize, ParametricCoefficientError> {
    let coefficient_capacity =
        parametric_vec_capacity_bound(terms, resource)?.max(minimum_coefficient_capacity);
    let exponent_capacity =
        parametric_vec_capacity_bound(exponent_entries, resource)?.max(minimum_exponent_capacity);
    let coefficient_slots =
        checked_parametric_mul(resource, coefficient_capacity, size_of::<Integer>())?;
    let exponent_payload = checked_parametric_mul(resource, exponent_capacity, size_of::<u16>())?;
    let per_integer_limb_payload = integer_limb_payload_byte_bound(integer_bits, resource)?
        .max(minimum_per_integer_payload_bytes);
    let integer_payload = checked_parametric_mul(resource, terms, per_integer_limb_payload)?;
    checked_parametric_add(
        resource,
        coefficient_slots,
        checked_parametric_add(resource, exponent_payload, integer_payload)?,
    )
}

fn integer_limb_payload_byte_bound(
    integer_bits: usize,
    resource: &'static str,
) -> Result<usize, ParametricCoefficientError> {
    if integer_bits == 0 {
        return Ok(0);
    }
    // GMP rounds capacity to whole limbs. One machine word beyond the exact
    // byte ceiling safely covers that final partial limb on supported targets.
    integer_bits
        .checked_add(7)
        .and_then(|bits| bits.checked_div(8))
        .and_then(|bytes| bytes.checked_add(size_of::<usize>()))
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
}

fn largest_integer_owned_capacity_bytes(
    polynomial: &CoefficientPolynomial,
) -> Result<usize, ParametricCoefficientError> {
    let mut largest = 0usize;
    for coefficient in &polynomial.coefficients {
        if let Integer::Large(value) = coefficient {
            let bytes = value
                .capacity()
                .checked_add(7)
                .and_then(|bits| bits.checked_div(8))
                .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "polynomial integer capacity bytes",
                })?;
            largest = largest.max(bytes);
        }
    }
    Ok(largest)
}

fn parametric_vec_capacity_bound(
    entries: usize,
    resource: &'static str,
) -> Result<usize, ParametricCoefficientError> {
    if entries == 0 {
        Ok(0)
    } else {
        // Symbolica sometimes allocates an exact non-power-of-two merge
        // buffer and then appends another monomial. Rust's amortized Vec
        // growth may double that prior capacity (for example 3 -> 6 while
        // retaining four entries), so next_power_of_two(entries) is not a
        // sound envelope. Every predecessor capacity is at most `entries`;
        // one final growth therefore retains at most twice that amount.
        entries
            .checked_mul(2)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
    }
}

fn polynomial_retained_bytes_with_wrapper(
    polynomial: &CoefficientPolynomial,
    wrapper_bytes: usize,
    resource: &'static str,
) -> Result<usize, ParametricCoefficientError> {
    checked_parametric_add(
        resource,
        wrapper_bytes,
        polynomial_owned_retained_byte_bound(polynomial)
            .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })?,
    )
}

fn verify_polynomial_execution_envelope(
    polynomial: &CoefficientPolynomial,
    term_bound: usize,
    exponent_entry_bound: usize,
    integer_bit_bound: usize,
    operation: &'static str,
) -> Result<(), ParametricCoefficientError> {
    if polynomial.nterms() > term_bound
        || polynomial.exponents.len() > exponent_entry_bound
        || polynomial
            .coefficients
            .iter()
            .any(|coefficient| integer_magnitude_bits(coefficient) > integer_bit_bound as u128)
    {
        return Err(ParametricCoefficientError::Symbolica(format!(
            "{operation} escaped its allocation-free preflight envelope"
        )));
    }
    Ok(())
}

fn verify_translated_coefficient_envelope(
    coefficient: &ParametricCoefficient,
    preflight: ParametricCoefficientTranslationPreflight,
) -> Result<(), ParametricCoefficientError> {
    let retained_terms = checked_parametric_add(
        "parametric translation normalized coefficient terms",
        coefficient.raw.numerator.nterms(),
        coefficient.raw.denominator.nterms(),
    )?;
    let retained_bytes = coefficient.owned_retained_byte_bound().ok_or(
        ParametricCoefficientError::ResourceCountOverflow {
            resource: "parametric translation normalized coefficient bytes",
        },
    )?;
    if retained_terms > preflight.normalized_coefficient_term_bound
        || retained_bytes > preflight.normalized_coefficient_byte_bound
    {
        return Err(ParametricCoefficientError::Symbolica(
            "parametric translation normalization escaped its preflight envelope".to_owned(),
        ));
    }
    Ok(())
}

fn verify_specialized_coefficient_envelope(
    coefficient: &Coefficient,
    guards: &[SpecializedNonZeroCondition],
    preflight: ParametricCoefficientSpecializationPreflight,
) -> Result<(), ParametricCoefficientError> {
    let retained_terms = checked_parametric_add(
        "coefficient specialization normalized coefficient terms",
        coefficient.numerator.nterms(),
        coefficient.denominator.nterms(),
    )?;
    let retained_bytes = checked_parametric_add(
        "coefficient specialization normalized coefficient bytes",
        size_of::<Coefficient>(),
        checked_parametric_add(
            "coefficient specialization normalized coefficient bytes",
            polynomial_owned_retained_byte_bound(&coefficient.numerator).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "coefficient specialization normalized coefficient bytes",
                },
            )?,
            polynomial_owned_retained_byte_bound(&coefficient.denominator).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "coefficient specialization normalized coefficient bytes",
                },
            )?,
        )?,
    )?;
    if retained_terms > preflight.normalized_coefficient_term_bound
        || retained_bytes > preflight.normalized_coefficient_byte_bound
        || guards.iter().any(|guard| {
            guard.polynomial.raw.nterms() > preflight.denominator_guard_term_bound
                || guard
                    .polynomial
                    .owned_retained_byte_bound()
                    .is_none_or(|bytes| bytes > preflight.denominator_guard_byte_bound)
        })
    {
        return Err(ParametricCoefficientError::Symbolica(
            "coefficient specialization normalization escaped its preflight envelope".to_owned(),
        ));
    }
    Ok(())
}

fn checked_parametric_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricCoefficientError> {
    left.checked_add(right)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
}

fn checked_parametric_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricCoefficientError> {
    left.checked_mul(right)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
}

fn parametric_ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResidualAffinePolynomialCompositionBackend {
    PolynomialEvaluator,
    SymbolicaExpressionExpansion,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResidualAffineSymbolicaExpressionIntegerPreflight {
    largest_integer_coefficient_bit_bound: usize,
    largest_integer_contribution_bit_bound: usize,
    integer_bit_work_bound: usize,
}

#[derive(Clone, Copy, Debug)]
struct ResidualUnitAffinePolynomialPreflight {
    stats: ResidualUnitAffinePolynomialCompositionStats,
    backend: ResidualAffinePolynomialCompositionBackend,
}

struct ResidualAffineCoefficientCorePreflight {
    numerator: ResidualUnitAffinePolynomialPreflight,
    denominator_limits: ResidualUnitAffinePolynomialCompositionLimits,
    denominator: ResidualUnitAffinePolynomialPreflight,
    stats: ResidualAffineCoefficientCompositionPreflight,
}

#[derive(Clone, Debug)]
struct ResidualAffineCoefficientHalves {
    numerator: ResidualAffinePolynomialComposition,
    denominator: ResidualAffinePolynomialComposition,
    stats: ResidualUnitAffineCoefficientCompositionStats,
}

#[derive(Clone, Debug)]
struct ResidualUnitAffineGuardPolynomialCopy {
    value: ParametricPolynomial,
    terms: usize,
    exponent_entries: usize,
    integer_bit_payload: usize,
}

#[derive(Clone, Debug)]
struct ResidualAffineCompactGeometry {
    ambient_arity: usize,
    free_positions: Vec<usize>,
    nonfree_positions: Vec<usize>,
    constants: Vec<Integer>,
    linear_coefficients: Vec<Integer>,
    geometry_entries_inspected: usize,
    geometry_entries_retained: usize,
    largest_image_integer_bits: usize,
    total_image_integer_bits: usize,
}

impl ResidualAffineCompactGeometry {
    fn linear_coefficient(&self, row: usize, free_ordinal: usize) -> Option<&Integer> {
        let offset = row
            .checked_mul(self.free_positions.len())?
            .checked_add(free_ordinal)?;
        self.linear_coefficients.get(offset)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResidualAffineCompactGeometryPreflight {
    composition: ResidualAffineCompositionPlanStats,
    geometry_integer_entries_inspected: usize,
    geometry_integer_bit_work: usize,
    geometry_replay_comparison_work: usize,
    geometry_replay_integer_bit_work: usize,
    geometry_replay_scratch_logical_bytes: usize,
    retained_owned_logical_bytes: usize,
    compilation_owned_logical_peak_upper_bound: usize,
    geometry_checksum: u64,
}

const RESIDUAL_AFFINE_DIAGNOSTIC_FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const RESIDUAL_AFFINE_DIAGNOSTIC_FNV1A64_PRIME: u64 = 0x100000001b3;

/// Deterministic diagnostic hasher.  Exact replay below, never this checksum,
/// authenticates a compact geometry.
struct ResidualAffineDiagnosticHasher(u64);

impl ResidualAffineDiagnosticHasher {
    const fn new() -> Self {
        Self(RESIDUAL_AFFINE_DIAGNOSTIC_FNV1A64_OFFSET)
    }

    fn write_usize(&mut self, value: usize) {
        self.write(&(value as u128).to_le_bytes());
    }
}

impl Hasher for ResidualAffineDiagnosticHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 ^= u64::from(byte);
            self.0 = self
                .0
                .wrapping_mul(RESIDUAL_AFFINE_DIAGNOSTIC_FNV1A64_PRIME);
        }
    }
}

fn residual_affine_diagnostic_checksum(bytes: &[u8]) -> u64 {
    let mut hasher = ResidualAffineDiagnosticHasher::new();
    hasher.write_usize(bytes.len());
    hasher.write(bytes);
    hasher.finish()
}

fn residual_affine_compact_geometry_checksum(geometry: ResidualAffineCompactMapView<'_>) -> u64 {
    let mut hasher = ResidualAffineDiagnosticHasher::new();
    hasher.write(RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA.as_bytes());
    hasher.write_usize(geometry.ambient_arity);
    hasher.write_usize(geometry.free_positions.len());
    for &position in geometry.free_positions {
        hasher.write_usize(position);
    }
    hasher.write_usize(geometry.constants.len());
    for value in geometry.constants {
        value.hash(&mut hasher);
    }
    hasher.write_usize(geometry.compact_linear_coefficients.len());
    for value in geometry.compact_linear_coefficients {
        value.hash(&mut hasher);
    }
    hasher.finish()
}

fn residual_affine_compact_geometry_error(
    reason: &'static str,
) -> ResidualUnitAffineCompositionError {
    ResidualUnitAffineCompositionError::InvalidCompactGeometry { reason }
}

fn validate_residual_affine_compact_support(
    geometry: ResidualAffineCompactMapView<'_>,
) -> Result<(), ResidualUnitAffineCompositionError> {
    let mut previous = None;
    for &position in geometry.free_positions {
        if position >= geometry.ambient_arity
            || previous.is_some_and(|previous| previous >= position)
        {
            return Err(residual_affine_compact_geometry_error(
                "free positions are not strictly sorted ambient positions",
            ));
        }
        previous = Some(position);
    }
    Ok(())
}

fn residual_affine_compact_geometry_preflight(
    context: &ParametricCoefficientContext,
    geometry: ResidualAffineCompactMapView<'_>,
    limits: ResidualAffineCompactCompositionPlanLimits,
) -> Result<ResidualAffineCompactGeometryPreflight, ResidualUnitAffineCompositionError> {
    check_residual_affine_limit(
        "compact affine context fingerprint bytes",
        context.fingerprint().len(),
        limits.max_context_fingerprint_bytes,
    )?;
    check_residual_affine_limit(
        "compact affine context fingerprint bytes",
        geometry.context_fingerprint.len(),
        limits.max_context_fingerprint_bytes,
    )?;
    if geometry.context_fingerprint != context.fingerprint() {
        return Err(ResidualUnitAffineCompositionError::WrongContext);
    }
    if geometry.ambient_arity != context.index_count() {
        return Err(ResidualUnitAffineCompositionError::WrongArity {
            expected: context.index_count(),
            actual: geometry.ambient_arity,
        });
    }
    validate_residual_affine_compact_support(geometry)?;
    if geometry.constants.len() != geometry.ambient_arity {
        return Err(residual_affine_compact_geometry_error(
            "constant vector does not match the ambient arity",
        ));
    }
    let compact_linear_entries = residual_affine_checked_mul(
        "compact affine linear coefficient shape",
        geometry.ambient_arity,
        geometry.free_positions.len(),
    )?;
    if geometry.compact_linear_coefficients.len() != compact_linear_entries {
        return Err(residual_affine_compact_geometry_error(
            "row-major compact linear matrix has the wrong shape",
        ));
    }
    let (geometry_entries_inspected, geometry_entries_retained) = residual_affine_geometry_counts(
        geometry.ambient_arity,
        geometry.free_positions.len(),
        false,
        limits.composition,
    )?;

    let base_identity_images = context.base.variables().len();
    let variable_count = base_identity_images
        .checked_add(geometry.ambient_arity)
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "composition variables",
        })?;
    check_residual_affine_limit(
        "composition variables",
        variable_count,
        limits.composition.max_variables,
    )?;
    check_residual_affine_limit(
        "full-point images",
        variable_count,
        limits.composition.max_full_images,
    )?;

    let (mut largest_image_integer_bits, mut total_image_integer_bits) =
        preflight_residual_affine_base_identity_images(base_identity_images, limits.composition)?;
    let mut geometry_integer_bit_work = 0usize;
    let mut geometry_large_payload_bytes = 0usize;
    let mut full_image_large_payload_bytes = 0usize;
    let mut total_image_terms = base_identity_images;
    let mut largest_image_owned_logical_bytes = usize::from(base_identity_images != 0)
        .checked_mul(size_of::<CoefficientPolynomial>())
        .and_then(|bytes| {
            bytes.checked_add(usize::from(base_identity_images != 0) * size_of::<Integer>())
        })
        .and_then(|bytes| {
            bytes.checked_add(
                usize::from(base_identity_images != 0)
                    .checked_mul(variable_count)?
                    .checked_mul(size_of::<u16>())?,
            )
        })
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "compact affine largest image logical bytes",
        })?;

    let mut next_free_ordinal = 0usize;
    for row in 0..geometry.ambient_arity {
        let constant = &geometry.constants[row];
        let row_free_ordinal = if geometry.free_positions.get(next_free_ordinal) == Some(&row) {
            let ordinal = next_free_ordinal;
            next_free_ordinal += 1;
            Some(ordinal)
        } else {
            None
        };
        if row_free_ordinal.is_some() && !constant.is_zero() {
            return Err(residual_affine_compact_geometry_error(
                "free affine row has nonzero translation",
            ));
        }
        preflight_residual_affine_image_integer(
            constant,
            &mut largest_image_integer_bits,
            &mut total_image_integer_bits,
            limits.composition,
        )?;
        let constant_bits = residual_affine_integer_bits(constant)?;
        geometry_integer_bit_work = residual_affine_checked_add(
            "compact affine geometry integer bit work",
            geometry_integer_bit_work,
            constant_bits,
        )?;
        geometry_large_payload_bytes = residual_affine_checked_add(
            "compact affine geometry logical bytes",
            geometry_large_payload_bytes,
            residual_affine_large_integer_dynamic_logical_bytes(
                constant,
                "compact affine geometry logical bytes",
            )?,
        )?;
        let mut row_terms = usize::from(!constant.is_zero());
        let mut row_large_payload_bytes = if constant.is_zero() {
            0
        } else {
            residual_affine_large_integer_dynamic_logical_bytes(
                constant,
                "compact affine image logical bytes",
            )?
        };
        full_image_large_payload_bytes = residual_affine_checked_add(
            "compact affine full-image logical bytes",
            full_image_large_payload_bytes,
            row_large_payload_bytes,
        )?;

        for free_ordinal in 0..geometry.free_positions.len() {
            let coefficient = geometry
                .linear_coefficient(row, free_ordinal)
                .ok_or_else(|| {
                    residual_affine_compact_geometry_error(
                        "row-major compact linear matrix has the wrong shape",
                    )
                })?;
            preflight_residual_affine_image_integer(
                coefficient,
                &mut largest_image_integer_bits,
                &mut total_image_integer_bits,
                limits.composition,
            )?;
            let coefficient_bits = residual_affine_integer_bits(coefficient)?;
            geometry_integer_bit_work = residual_affine_checked_add(
                "compact affine geometry integer bit work",
                geometry_integer_bit_work,
                coefficient_bits,
            )?;
            geometry_large_payload_bytes = residual_affine_checked_add(
                "compact affine geometry logical bytes",
                geometry_large_payload_bytes,
                residual_affine_large_integer_dynamic_logical_bytes(
                    coefficient,
                    "compact affine geometry logical bytes",
                )?,
            )?;
            if let Some(row_free_ordinal) = row_free_ordinal {
                let expected = free_ordinal == row_free_ordinal;
                if (expected && coefficient != &Integer::one())
                    || (!expected && !coefficient.is_zero())
                {
                    return Err(residual_affine_compact_geometry_error(
                        "free affine row is not an identity row",
                    ));
                }
            }
            if !coefficient.is_zero() {
                row_terms =
                    residual_affine_checked_add("compact affine image terms", row_terms, 1)?;
                row_large_payload_bytes = residual_affine_checked_add(
                    "compact affine image logical bytes",
                    row_large_payload_bytes,
                    residual_affine_large_integer_dynamic_logical_bytes(
                        coefficient,
                        "compact affine image logical bytes",
                    )?,
                )?;
                full_image_large_payload_bytes = residual_affine_checked_add(
                    "compact affine full-image logical bytes",
                    full_image_large_payload_bytes,
                    residual_affine_large_integer_dynamic_logical_bytes(
                        coefficient,
                        "compact affine full-image logical bytes",
                    )?,
                )?;
            }
        }

        total_image_terms =
            residual_affine_checked_add("total image terms", total_image_terms, row_terms)?;
        check_residual_affine_limit(
            "total image terms",
            total_image_terms,
            limits.composition.max_total_image_terms,
        )?;
        let row_exponent_entries = residual_affine_checked_mul(
            "compact affine image logical bytes",
            row_terms,
            variable_count,
        )?;
        let row_dynamic_bytes = [
            size_of::<CoefficientPolynomial>(),
            residual_affine_checked_mul(
                "compact affine image logical bytes",
                row_terms,
                size_of::<Integer>(),
            )?,
            residual_affine_checked_mul(
                "compact affine image logical bytes",
                row_exponent_entries,
                size_of::<u16>(),
            )?,
            row_large_payload_bytes,
        ]
        .into_iter()
        .try_fold(0usize, |sum, bytes| {
            residual_affine_checked_add("compact affine image logical bytes", sum, bytes)
        })?;
        largest_image_owned_logical_bytes =
            largest_image_owned_logical_bytes.max(row_dynamic_bytes);
    }
    if next_free_ordinal != geometry.free_positions.len() {
        return Err(residual_affine_compact_geometry_error(
            "free affine row lookup failed",
        ));
    }
    check_residual_affine_limit(
        "compact affine geometry integer bit work",
        geometry_integer_bit_work,
        limits.max_geometry_integer_bit_work,
    )?;

    let total_image_exponent_entries = residual_affine_checked_mul(
        "total image exponent entries",
        total_image_terms,
        variable_count,
    )?;
    check_residual_affine_limit(
        "total image exponent entries",
        total_image_exponent_entries,
        limits.composition.max_total_image_exponent_entries,
    )?;
    let support_entries_retained = geometry_entries_retained;
    let composition = ResidualAffineCompositionPlanStats {
        variables: variable_count,
        full_images: variable_count,
        geometry_entries_inspected,
        geometry_entries_retained,
        support_entries_retained,
        total_image_terms,
        total_image_exponent_entries,
        largest_image_integer_bits,
        total_image_integer_bits,
    };

    // Exact successful-path work for the single-pass deep replay below. The
    // two lookup vectors make term classification O(1) after each dense
    // exponent vector has been inspected once; no image term is rescanned for
    // each expected affine coefficient.
    let free_slot_count = residual_affine_checked_add(
        "compact affine replay free slots",
        geometry.free_positions.len(),
        1,
    )?;
    let replay_lookup_initializations = [
        geometry.ambient_arity,
        geometry.free_positions.len(),
        free_slot_count,
    ]
    .into_iter()
    .try_fold(0usize, |sum, count| {
        residual_affine_checked_add("compact affine replay comparison work", sum, count)
    })?;
    let geometry_replay_comparison_work = [
        replay_lookup_initializations,
        geometry.ambient_arity,
        variable_count,
        geometry_entries_retained,
        total_image_terms,
        total_image_exponent_entries,
    ]
    .into_iter()
    .try_fold(0usize, |sum, count| {
        residual_affine_checked_add("compact affine replay comparison work", sum, count)
    })?;
    check_residual_affine_limit(
        "compact affine replay comparison work",
        geometry_replay_comparison_work,
        limits.max_geometry_replay_comparison_work,
    )?;
    let geometry_replay_integer_bit_work = residual_affine_checked_add(
        "compact affine replay integer bit work",
        geometry_integer_bit_work,
        total_image_integer_bits,
    )?;
    check_residual_affine_limit(
        "compact affine replay integer bit work",
        geometry_replay_integer_bit_work,
        limits.max_geometry_replay_integer_bit_work,
    )?;
    let geometry_replay_scratch_entries = residual_affine_checked_add(
        "compact affine replay scratch logical bytes",
        geometry.ambient_arity,
        free_slot_count,
    )?;
    let geometry_replay_scratch_logical_bytes = residual_affine_checked_add(
        "compact affine replay scratch logical bytes",
        residual_affine_checked_mul(
            "compact affine replay scratch logical bytes",
            2,
            size_of::<Vec<usize>>(),
        )?,
        residual_affine_checked_mul(
            "compact affine replay scratch logical bytes",
            geometry_replay_scratch_entries,
            size_of::<usize>(),
        )?,
    )?;
    check_residual_affine_limit(
        "compact affine replay scratch logical bytes",
        geometry_replay_scratch_logical_bytes,
        limits.max_geometry_replay_scratch_logical_bytes,
    )?;

    let resource = "compact affine composition plan logical bytes";
    let linear_support_bytes = compact_linear_entries
        .checked_add(u8::BITS as usize - 1)
        .and_then(|bits| bits.checked_div(u8::BITS as usize))
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?;
    let full_image_dynamic_bytes = [
        residual_affine_checked_mul(resource, total_image_terms, size_of::<Integer>())?,
        residual_affine_checked_mul(resource, total_image_exponent_entries, size_of::<u16>())?,
        full_image_large_payload_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let retained_owned_logical_bytes = [
        size_of::<ResidualAffineCompactCompositionPlan>(),
        arc_payload_control_and_padding_byte_bound::<ResidualAffineCompositionCorePlan>()
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })?,
        residual_affine_checked_mul(resource, geometry.ambient_arity, size_of::<usize>())?,
        linear_support_bytes,
        residual_affine_checked_mul(resource, variable_count, size_of::<CoefficientPolynomial>())?,
        full_image_dynamic_bytes,
        residual_affine_checked_mul(
            resource,
            residual_affine_checked_mul(resource, variable_count, 2)?,
            size_of::<usize>(),
        )?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    check_residual_affine_limit(
        "compact affine retained owned logical bytes",
        retained_owned_logical_bytes,
        limits.max_retained_owned_logical_bytes,
    )?;

    let compact_geometry_bytes = [
        size_of::<ResidualAffineCompactGeometry>(),
        residual_affine_checked_mul(resource, geometry.ambient_arity, size_of::<usize>())?,
        residual_affine_checked_mul(resource, geometry_entries_retained, size_of::<Integer>())?,
        geometry_large_payload_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    let exponent_scratch_bytes = residual_affine_checked_add(
        resource,
        size_of::<Vec<u16>>(),
        residual_affine_checked_mul(resource, variable_count, size_of::<u16>())?,
    )?;
    let compilation_owned_logical_peak_upper_bound = [
        retained_owned_logical_bytes,
        compact_geometry_bytes,
        exponent_scratch_bytes,
        largest_image_owned_logical_bytes,
        geometry_replay_scratch_logical_bytes,
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        residual_affine_checked_add(resource, sum, bytes)
    })?;
    check_residual_affine_limit(
        "compact affine compilation owned logical peak upper bound",
        compilation_owned_logical_peak_upper_bound,
        limits.max_compilation_owned_logical_peak_upper_bound,
    )?;

    Ok(ResidualAffineCompactGeometryPreflight {
        composition,
        geometry_integer_entries_inspected: geometry_entries_inspected,
        geometry_integer_bit_work,
        geometry_replay_comparison_work,
        geometry_replay_integer_bit_work,
        geometry_replay_scratch_logical_bytes,
        retained_owned_logical_bytes,
        compilation_owned_logical_peak_upper_bound,
        geometry_checksum: residual_affine_compact_geometry_checksum(geometry),
    })
}

fn materialize_residual_affine_compact_geometry(
    geometry: ResidualAffineCompactMapView<'_>,
    preflight: ResidualAffineCompactGeometryPreflight,
) -> Result<ResidualAffineCompactGeometry, ResidualUnitAffineCompositionError> {
    let nonfree_count = geometry
        .ambient_arity
        .checked_sub(geometry.free_positions.len())
        .ok_or_else(|| {
            residual_affine_compact_geometry_error(
                "free positions exceed the ambient coordinate count",
            )
        })?;
    let mut free_positions = Vec::new();
    free_positions
        .try_reserve_exact(geometry.free_positions.len())
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine free positions",
            requested: geometry.free_positions.len(),
        })?;
    free_positions.extend_from_slice(geometry.free_positions);
    let mut nonfree_positions = Vec::new();
    nonfree_positions
        .try_reserve_exact(nonfree_count)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine nonfree positions",
            requested: nonfree_count,
        })?;
    let mut free_ordinal = 0usize;
    for position in 0..geometry.ambient_arity {
        if geometry.free_positions.get(free_ordinal) == Some(&position) {
            free_ordinal += 1;
        } else {
            nonfree_positions.push(position);
        }
    }
    let mut constants = Vec::new();
    constants
        .try_reserve_exact(geometry.constants.len())
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine constants",
            requested: geometry.constants.len(),
        })?;
    constants.extend(geometry.constants.iter().cloned());
    let mut linear_coefficients = Vec::new();
    linear_coefficients
        .try_reserve_exact(geometry.compact_linear_coefficients.len())
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine linear coefficients",
            requested: geometry.compact_linear_coefficients.len(),
        })?;
    linear_coefficients.extend(geometry.compact_linear_coefficients.iter().cloned());
    Ok(ResidualAffineCompactGeometry {
        ambient_arity: geometry.ambient_arity,
        free_positions,
        nonfree_positions,
        constants,
        linear_coefficients,
        geometry_entries_inspected: preflight.composition.geometry_entries_inspected,
        geometry_entries_retained: preflight.composition.geometry_entries_retained,
        largest_image_integer_bits: preflight.composition.largest_image_integer_bits,
        total_image_integer_bits: preflight.composition.total_image_integer_bits,
    })
}

fn residual_affine_compact_image_term_matches(
    image: &CoefficientPolynomial,
    term_ordinal: usize,
    expected_coefficient: &Integer,
    target_variable: Option<usize>,
    variable_count: usize,
) -> bool {
    if image.coefficients.get(term_ordinal) != Some(expected_coefficient) {
        return false;
    }
    let Some(start) = term_ordinal.checked_mul(variable_count) else {
        return false;
    };
    let Some(end) = start.checked_add(variable_count) else {
        return false;
    };
    let Some(exponents) = image.exponents.get(start..end) else {
        return false;
    };
    exponents
        .iter()
        .enumerate()
        .all(|(variable, &exponent)| exponent == u16::from(target_variable == Some(variable)))
}

fn residual_affine_compact_image_term_slot(
    image: &CoefficientPolynomial,
    term_ordinal: usize,
    variable_count: usize,
    base_count: usize,
    free_ordinal_by_ambient_position: &[usize],
) -> Option<usize> {
    let start = term_ordinal.checked_mul(variable_count)?;
    let end = start.checked_add(variable_count)?;
    let exponents = image.exponents.get(start..end)?;
    let mut target_variable = None;
    for (variable, &exponent) in exponents.iter().enumerate() {
        match exponent {
            0 => {}
            1 if target_variable.is_none() => target_variable = Some(variable),
            _ => return None,
        }
    }
    let Some(target_variable) = target_variable else {
        return Some(0);
    };
    let ambient_position = target_variable.checked_sub(base_count)?;
    let free_ordinal = *free_ordinal_by_ambient_position.get(ambient_position)?;
    if free_ordinal == usize::MAX {
        None
    } else {
        free_ordinal.checked_add(1)
    }
}

fn replay_residual_affine_compact_geometry_against_core(
    context: &ParametricCoefficientContext,
    core: &ResidualAffineCompositionCorePlan,
    geometry: ResidualAffineCompactMapView<'_>,
    preflight: ResidualAffineCompactGeometryPreflight,
) -> Result<(), ResidualUnitAffineCompositionError> {
    if core.ambient_arity != geometry.ambient_arity
        || core.free_positions.as_slice() != geometry.free_positions
        || core.stats != preflight.composition
    {
        return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
    }
    let base_count = context.base.variables().len();
    let variable_count = base_count.checked_add(geometry.ambient_arity).ok_or(
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "compact affine replay variables",
        },
    )?;
    if core.full_images.len() != variable_count
        || core.image_term_counts.len() != variable_count
        || core.image_coefficient_growth_bits.len() != variable_count
    {
        return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
    }

    let mut expected_nonfree_ordinal = 0usize;
    let mut free_ordinal = 0usize;
    for position in 0..geometry.ambient_arity {
        if geometry.free_positions.get(free_ordinal) == Some(&position) {
            free_ordinal += 1;
        } else {
            if core.nonfree_positions.get(expected_nonfree_ordinal) != Some(&position) {
                return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
            }
            expected_nonfree_ordinal += 1;
        }
    }
    if expected_nonfree_ordinal != core.nonfree_positions.len() {
        return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
    }

    // These two bounded lookup tables turn deep replay into one pass over the
    // canonical dense exponent payload. The inverse table classifies a unit
    // monomial in O(1); generation marks reject duplicate monomials without a
    // per-row clear or a nested rescan of the Symbolica image.
    let mut free_ordinal_by_ambient_position = Vec::new();
    free_ordinal_by_ambient_position
        .try_reserve_exact(geometry.ambient_arity)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine replay ambient lookup",
            requested: geometry.ambient_arity,
        })?;
    free_ordinal_by_ambient_position.resize(geometry.ambient_arity, usize::MAX);
    for (free_ordinal, &position) in geometry.free_positions.iter().enumerate() {
        let slot = free_ordinal_by_ambient_position
            .get_mut(position)
            .ok_or(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)?;
        if *slot != usize::MAX {
            return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
        }
        *slot = free_ordinal;
    }
    let free_slot_count = geometry.free_positions.len().checked_add(1).ok_or(
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "compact affine replay free slots",
        },
    )?;
    let mut seen_generation = Vec::new();
    seen_generation
        .try_reserve_exact(free_slot_count)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine replay occurrence generations",
            requested: free_slot_count,
        })?;
    seen_generation.resize(free_slot_count, 0usize);

    for variable in 0..base_count {
        let image = &core.full_images[variable];
        if !Arc::ptr_eq(&image.variables, &context.variables)
            || image.coefficients.len() != 1
            || image.exponents.len() != variable_count
            || !residual_affine_compact_image_term_matches(
                image,
                0,
                &Integer::one(),
                Some(variable),
                variable_count,
            )
            || core.image_term_counts[variable] != 1
            || core.image_coefficient_growth_bits[variable] != 0
        {
            return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
        }
    }

    for row in 0..geometry.ambient_arity {
        let image_ordinal = base_count + row;
        let image = &core.full_images[image_ordinal];
        if !Arc::ptr_eq(&image.variables, &context.variables) {
            return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
        }
        let constant = &geometry.constants[row];
        let mut expected_terms = 0usize;
        let mut expected_growth_bits = residual_affine_integer_growth_bits(constant)?;
        if !constant.is_zero() {
            expected_terms += 1;
        }
        for target_free_ordinal in 0..geometry.free_positions.len() {
            let coefficient = geometry
                .linear_coefficient(row, target_free_ordinal)
                .ok_or(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)?;
            if core.linear_is_nonzero(row, target_free_ordinal) != Some(!coefficient.is_zero()) {
                return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
            }
            expected_growth_bits =
                expected_growth_bits.max(residual_affine_integer_growth_bits(coefficient)?);
            if coefficient.is_zero() {
                continue;
            }
            expected_terms += 1;
        }
        let expected_exponent_entries = residual_affine_checked_mul(
            "compact affine replay image exponents",
            expected_terms,
            variable_count,
        )?;
        if image.coefficients.len() != expected_terms
            || image.exponents.len() != expected_exponent_entries
            || core.image_term_counts[image_ordinal] != expected_terms
            || core.image_coefficient_growth_bits[image_ordinal] != expected_growth_bits
        {
            return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
        }
        let generation = row.checked_add(1).ok_or(
            ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "compact affine replay occurrence generation",
            },
        )?;
        for (term_ordinal, observed_coefficient) in image.coefficients.iter().enumerate() {
            let slot = residual_affine_compact_image_term_slot(
                image,
                term_ordinal,
                variable_count,
                base_count,
                &free_ordinal_by_ambient_position,
            )
            .ok_or(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)?;
            let expected_coefficient = if slot == 0 {
                constant
            } else {
                geometry
                    .linear_coefficient(row, slot - 1)
                    .ok_or(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)?
            };
            let seen = seen_generation
                .get_mut(slot)
                .ok_or(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)?;
            if expected_coefficient.is_zero()
                || *seen == generation
                || observed_coefficient != expected_coefficient
            {
                return Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch);
            }
            *seen = generation;
        }
    }
    Ok(())
}

fn residual_affine_legacy_geometry_error(_: &'static str) -> ResidualUnitAffineCompositionError {
    ResidualUnitAffineCompositionError::Map(ResidualUnitAffineIndexMapError::ReplayMismatch)
}

fn residual_affine_integer_geometry_error(
    message: &'static str,
) -> ResidualUnitAffineCompositionError {
    ResidualUnitAffineCompositionError::IntegerSystem(
        ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(message),
    )
}

fn normalize_residual_affine_partition(
    arity: usize,
    mut nonfree_positions: Vec<usize>,
    source_free_positions: &[usize],
    limits: ResidualUnitAffineCompositionPlanLimits,
    invalid: fn(&'static str) -> ResidualUnitAffineCompositionError,
) -> Result<(Vec<usize>, Vec<usize>), ResidualUnitAffineCompositionError> {
    let support_entries = nonfree_positions
        .len()
        .checked_add(source_free_positions.len())
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "affine support entries retained",
        })?;
    check_residual_affine_limit(
        "affine support entries retained",
        support_entries,
        limits.max_support_entries_retained,
    )?;
    if support_entries != arity {
        return Err(invalid(
            "pivot/free positions do not partition the ambient coordinates",
        ));
    }

    let mut free_positions = Vec::new();
    free_positions
        .try_reserve_exact(source_free_positions.len())
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "free support positions",
            requested: source_free_positions.len(),
        })?;
    let mut previous_free = None;
    for &position in source_free_positions {
        if position >= arity || previous_free.is_some_and(|previous| previous >= position) {
            return Err(invalid(
                "free positions are not strictly sorted ambient positions",
            ));
        }
        free_positions.push(position);
        previous_free = Some(position);
    }

    nonfree_positions.sort_unstable();
    let mut previous_nonfree = None;
    for &position in &nonfree_positions {
        if position >= arity || previous_nonfree == Some(position) {
            return Err(invalid(
                "pivot positions are not distinct ambient positions",
            ));
        }
        previous_nonfree = Some(position);
    }

    let mut nonfree_ordinal = 0usize;
    let mut free_ordinal = 0usize;
    for position in 0..arity {
        let is_nonfree = nonfree_positions.get(nonfree_ordinal) == Some(&position);
        let is_free = free_positions.get(free_ordinal) == Some(&position);
        if is_nonfree == is_free {
            return Err(invalid(
                "pivot/free positions are not a disjoint complete partition",
            ));
        }
        nonfree_ordinal += usize::from(is_nonfree);
        free_ordinal += usize::from(is_free);
    }
    Ok((nonfree_positions, free_positions))
}

fn residual_affine_geometry_counts(
    arity: usize,
    free_count: usize,
    ambient_square: bool,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<(usize, usize), ResidualUnitAffineCompositionError> {
    let retained_linear =
        residual_affine_checked_mul("affine geometry entries retained", arity, free_count)?;
    let retained =
        residual_affine_checked_add("affine geometry entries retained", arity, retained_linear)?;
    let inspected_linear = if ambient_square {
        residual_affine_checked_mul("affine geometry entries inspected", arity, arity)?
    } else {
        retained_linear
    };
    let inspected =
        residual_affine_checked_add("affine geometry entries inspected", arity, inspected_linear)?;
    check_residual_affine_limit(
        "affine geometry entries inspected",
        inspected,
        limits.max_geometry_entries_inspected,
    )?;
    check_residual_affine_limit(
        "affine geometry entries retained",
        retained,
        limits.max_geometry_entries_retained,
    )?;
    // The durable support is one position entry per ambient coordinate plus
    // one Boolean entry per compact linear coefficient.  Cap it before the
    // transient GMP geometry is cloned.
    check_residual_affine_limit(
        "affine support entries retained",
        retained,
        limits.max_support_entries_retained,
    )?;
    Ok((inspected, retained))
}

fn preflight_residual_affine_image_integer(
    value: &Integer,
    largest: &mut usize,
    total: &mut usize,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<(), ResidualUnitAffineCompositionError> {
    let bits = residual_affine_integer_bits(value)?;
    check_residual_affine_limit(
        "image integer coefficient bits",
        bits,
        limits.max_image_integer_bits,
    )?;
    *largest = (*largest).max(bits);
    *total = residual_affine_checked_add("total image integer bits", *total, bits)?;
    check_residual_affine_limit(
        "total image integer bits",
        *total,
        limits.max_total_image_integer_bits,
    )
}

fn preflight_residual_affine_base_identity_images(
    image_count: usize,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<(usize, usize), ResidualUnitAffineCompositionError> {
    if image_count == 0 {
        return Ok((0, 0));
    }
    let unit_bits = residual_affine_integer_bits(&Integer::one())?;
    check_residual_affine_limit(
        "image integer coefficient bits",
        unit_bits,
        limits.max_image_integer_bits,
    )?;
    let total_bits =
        residual_affine_checked_mul("total image integer bits", image_count, unit_bits)?;
    check_residual_affine_limit(
        "total image integer bits",
        total_bits,
        limits.max_total_image_integer_bits,
    )?;
    Ok((unit_bits, total_bits))
}

fn compact_legacy_unit_affine_geometry(
    map: &ResidualUnitAffineIndexMapCertificate,
    arity: usize,
    base_identity_images: usize,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<ResidualAffineCompactGeometry, ResidualUnitAffineCompositionError> {
    if map.ambient_arity() != arity {
        return Err(ResidualUnitAffineCompositionError::WrongArity {
            expected: arity,
            actual: map.ambient_arity(),
        });
    }
    let nonfree_count = map.literal_positions().len().checked_add(1).ok_or(
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "pivot support positions",
        },
    )?;
    let source_support_count = nonfree_count
        .checked_add(map.free_positions().len())
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "affine support entries retained",
        })?;
    check_residual_affine_limit(
        "affine support entries retained",
        source_support_count,
        limits.max_support_entries_retained,
    )?;
    if source_support_count != arity {
        return Err(residual_affine_legacy_geometry_error(
            "pivot/free positions do not partition the ambient coordinates",
        ));
    }
    let mut source_nonfree = Vec::new();
    source_nonfree
        .try_reserve_exact(nonfree_count)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "pivot support positions",
            requested: nonfree_count,
        })?;
    source_nonfree.extend(map.literal_positions().iter().copied());
    source_nonfree.push(map.bound_position());
    let (nonfree_positions, free_positions) = normalize_residual_affine_partition(
        arity,
        source_nonfree,
        map.free_positions(),
        limits,
        residual_affine_legacy_geometry_error,
    )?;
    let (geometry_entries_inspected, geometry_entries_retained) =
        residual_affine_geometry_counts(arity, free_positions.len(), false, limits)?;

    let (mut largest_image_integer_bits, mut total_image_integer_bits) =
        preflight_residual_affine_base_identity_images(base_identity_images, limits)?;
    for row in 0..arity {
        let constant = map
            .constant(row)
            .ok_or_else(|| residual_affine_legacy_geometry_error("missing affine constant"))?;
        preflight_residual_affine_image_integer(
            constant,
            &mut largest_image_integer_bits,
            &mut total_image_integer_bits,
            limits,
        )?;
        for free_ordinal in 0..free_positions.len() {
            let coefficient = map.linear_coefficient(row, free_ordinal).ok_or_else(|| {
                residual_affine_legacy_geometry_error("missing compact affine coefficient")
            })?;
            preflight_residual_affine_image_integer(
                coefficient,
                &mut largest_image_integer_bits,
                &mut total_image_integer_bits,
                limits,
            )?;
        }
    }
    for (free_ordinal, &free_position) in free_positions.iter().enumerate() {
        if !map.constant(free_position).is_some_and(Integer::is_zero) {
            return Err(residual_affine_legacy_geometry_error(
                "free affine row has nonzero translation",
            ));
        }
        for target_free_ordinal in 0..free_positions.len() {
            let expected = if target_free_ordinal == free_ordinal {
                Integer::one()
            } else {
                Integer::zero()
            };
            if map.linear_coefficient(free_position, target_free_ordinal) != Some(&expected) {
                return Err(residual_affine_legacy_geometry_error(
                    "free affine row is not an identity row",
                ));
            }
        }
    }

    let mut constants = Vec::new();
    constants.try_reserve_exact(arity).map_err(|_| {
        ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine constants",
            requested: arity,
        }
    })?;
    let linear_count = geometry_entries_retained.checked_sub(arity).ok_or(
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "compact affine linear coefficients",
        },
    )?;
    let mut linear_coefficients = Vec::new();
    linear_coefficients
        .try_reserve_exact(linear_count)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine linear coefficients",
            requested: linear_count,
        })?;
    for row in 0..arity {
        constants.push(
            map.constant(row)
                .ok_or_else(|| residual_affine_legacy_geometry_error("missing affine constant"))?
                .clone(),
        );
        for free_ordinal in 0..free_positions.len() {
            linear_coefficients.push(
                map.linear_coefficient(row, free_ordinal)
                    .ok_or_else(|| {
                        residual_affine_legacy_geometry_error("missing compact affine coefficient")
                    })?
                    .clone(),
            );
        }
    }
    Ok(ResidualAffineCompactGeometry {
        ambient_arity: arity,
        free_positions,
        nonfree_positions,
        constants,
        linear_coefficients,
        geometry_entries_inspected,
        geometry_entries_retained,
        largest_image_integer_bits,
        total_image_integer_bits,
    })
}

fn compact_integer_system_affine_geometry(
    map: &ResidualAffineIntegerMap,
    arity: usize,
    base_identity_images: usize,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<ResidualAffineCompactGeometry, ResidualUnitAffineCompositionError> {
    compact_ambient_square_affine_geometry_from_accessors(
        map.ambient_arity(),
        arity,
        map.pivot_positions(),
        map.free_positions(),
        |row| map.constant(row),
        |row, column| map.linear_coefficient(row, column),
        base_identity_images,
        limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn compact_ambient_square_affine_geometry_from_accessors<'a>(
    actual_arity: usize,
    expected_arity: usize,
    source_nonfree_positions: &[usize],
    source_free_positions: &[usize],
    constant: impl Fn(usize) -> Option<&'a Integer>,
    linear_coefficient: impl Fn(usize, usize) -> Option<&'a Integer>,
    base_identity_images: usize,
    limits: ResidualUnitAffineCompositionPlanLimits,
) -> Result<ResidualAffineCompactGeometry, ResidualUnitAffineCompositionError> {
    if actual_arity != expected_arity {
        return Err(ResidualUnitAffineCompositionError::WrongArity {
            expected: expected_arity,
            actual: actual_arity,
        });
    }
    let source_support_count = source_nonfree_positions
        .len()
        .checked_add(source_free_positions.len())
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "affine support entries retained",
        })?;
    check_residual_affine_limit(
        "affine support entries retained",
        source_support_count,
        limits.max_support_entries_retained,
    )?;
    if source_support_count != expected_arity {
        return Err(residual_affine_integer_geometry_error(
            "pivot/free positions do not partition the ambient coordinates",
        ));
    }
    let mut source_nonfree = Vec::new();
    source_nonfree
        .try_reserve_exact(source_nonfree_positions.len())
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "pivot support positions",
            requested: source_nonfree_positions.len(),
        })?;
    source_nonfree.extend(source_nonfree_positions.iter().copied());
    let (nonfree_positions, free_positions) = normalize_residual_affine_partition(
        expected_arity,
        source_nonfree,
        source_free_positions,
        limits,
        residual_affine_integer_geometry_error,
    )?;
    let (geometry_entries_inspected, geometry_entries_retained) =
        residual_affine_geometry_counts(expected_arity, free_positions.len(), true, limits)?;

    let (mut largest_image_integer_bits, mut total_image_integer_bits) =
        preflight_residual_affine_base_identity_images(base_identity_images, limits)?;
    for row in 0..expected_arity {
        let row_constant = constant(row).ok_or_else(|| {
            residual_affine_integer_geometry_error("ambient affine map is missing a constant")
        })?;
        preflight_residual_affine_image_integer(
            row_constant,
            &mut largest_image_integer_bits,
            &mut total_image_integer_bits,
            limits,
        )?;
        for column in 0..expected_arity {
            let coefficient = linear_coefficient(row, column).ok_or_else(|| {
                residual_affine_integer_geometry_error(
                    "ambient affine map is missing a square-matrix coefficient",
                )
            })?;
            if nonfree_positions.binary_search(&column).is_ok() && !coefficient.is_zero() {
                return Err(residual_affine_integer_geometry_error(
                    "ambient affine map has a nonzero nonfree column",
                ));
            }
            if let Ok(free_ordinal) = free_positions.binary_search(&column) {
                preflight_residual_affine_image_integer(
                    coefficient,
                    &mut largest_image_integer_bits,
                    &mut total_image_integer_bits,
                    limits,
                )?;
                if row == free_positions[free_ordinal] {
                    if coefficient != &Integer::one() {
                        return Err(residual_affine_integer_geometry_error(
                            "free ambient affine row is not an identity row",
                        ));
                    }
                } else if free_positions.binary_search(&row).is_ok() && !coefficient.is_zero() {
                    return Err(residual_affine_integer_geometry_error(
                        "free ambient affine row is not an identity row",
                    ));
                }
            }
        }
    }
    for &free_position in &free_positions {
        if !constant(free_position).is_some_and(Integer::is_zero) {
            return Err(residual_affine_integer_geometry_error(
                "free ambient affine row has nonzero translation",
            ));
        }
    }

    let mut constants = Vec::new();
    constants.try_reserve_exact(expected_arity).map_err(|_| {
        ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine constants",
            requested: expected_arity,
        }
    })?;
    let linear_count = geometry_entries_retained
        .checked_sub(expected_arity)
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "compact affine linear coefficients",
        })?;
    let mut linear_coefficients = Vec::new();
    linear_coefficients
        .try_reserve_exact(linear_count)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource: "compact affine linear coefficients",
            requested: linear_count,
        })?;
    for row in 0..expected_arity {
        constants.push(
            constant(row)
                .ok_or_else(|| {
                    residual_affine_integer_geometry_error(
                        "ambient affine map is missing a constant",
                    )
                })?
                .clone(),
        );
        for &free_position in &free_positions {
            // This is the essential ambient-to-compact adapter.  `free_position`
            // is an original coordinate, never the compact ordinal.
            linear_coefficients.push(
                linear_coefficient(row, free_position)
                    .ok_or_else(|| {
                        residual_affine_integer_geometry_error(
                            "ambient affine map is missing a square-matrix coefficient",
                        )
                    })?
                    .clone(),
            );
        }
    }
    Ok(ResidualAffineCompactGeometry {
        ambient_arity: expected_arity,
        free_positions,
        nonfree_positions,
        constants,
        linear_coefficients,
        geometry_entries_inspected,
        geometry_entries_retained,
        largest_image_integer_bits,
        total_image_integer_bits,
    })
}

fn reserve_residual_affine_polynomial(
    template: &CoefficientPolynomial,
    terms: usize,
    variables: usize,
    resource: &'static str,
) -> Result<CoefficientPolynomial, ResidualUnitAffineCompositionError> {
    let exponent_entries = residual_affine_checked_mul(resource, terms, variables)?;
    let mut result = template.zero();
    result.coefficients.try_reserve_exact(terms).map_err(|_| {
        ResidualUnitAffineCompositionError::AllocationFailure {
            resource,
            requested: terms,
        }
    })?;
    result
        .exponents
        .try_reserve_exact(exponent_entries)
        .map_err(|_| ResidualUnitAffineCompositionError::AllocationFailure {
            resource,
            requested: exponent_entries,
        })?;
    Ok(result)
}

fn residual_affine_guard_locator(
    map: &ResidualUnitAffineIndexMapCertificate,
) -> (u64, usize, usize) {
    (
        map.source_case().value(),
        map.source_equality_predicate_ordinal(),
        map.bound_position(),
    )
}

fn residual_affine_remaining_limits(
    limits: ResidualUnitAffinePolynomialCompositionLimits,
    consumed: &ResidualUnitAffinePolynomialPreflight,
) -> Result<ResidualUnitAffinePolynomialCompositionLimits, ResidualUnitAffineCompositionError> {
    let subtract = |resource: &'static str, limit: usize, used: usize| {
        limit
            .checked_sub(used)
            .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })
    };
    Ok(ResidualUnitAffinePolynomialCompositionLimits {
        exact_algebra: limits.exact_algebra,
        max_source_terms: subtract(
            "aggregate source terms",
            limits.max_source_terms,
            consumed.stats.source_terms,
        )?,
        max_source_exponent_entries: subtract(
            "aggregate source exponent entries",
            limits.max_source_exponent_entries,
            consumed.stats.source_exponent_entries,
        )?,
        max_expanded_contributions: subtract(
            "aggregate expanded contributions",
            limits.max_expanded_contributions,
            consumed.stats.expanded_contribution_bound,
        )?,
        max_output_terms: subtract(
            "aggregate prospective output terms",
            limits.max_output_terms,
            consumed.stats.expanded_contribution_bound,
        )?,
        max_output_exponent_entries: subtract(
            "aggregate prospective output exponent entries",
            limits.max_output_exponent_entries,
            consumed.stats.output_exponent_entry_bound,
        )?,
        max_power_calls: subtract(
            "aggregate native power calls",
            limits.max_power_calls,
            consumed.stats.power_calls,
        )?,
        max_native_power_heap_pairs: subtract(
            "aggregate native power heap pairs",
            limits.max_native_power_heap_pairs,
            consumed.stats.native_power_heap_pair_bound,
        )?,
        max_multiplication_term_pairs: subtract(
            "aggregate native multiplication term pairs",
            limits.max_multiplication_term_pairs,
            consumed.stats.multiplication_term_pair_bound,
        )?,
        max_addition_term_visits: subtract(
            "aggregate Symbolica structural term visits",
            limits.max_addition_term_visits,
            consumed.stats.addition_term_visit_bound,
        )?,
        max_kronecker_exponent_bits: limits.max_kronecker_exponent_bits,
        max_integer_coefficient_bits: limits.max_integer_coefficient_bits,
        max_integer_bit_work: subtract(
            "aggregate integer bit work",
            limits.max_integer_bit_work,
            consumed.stats.integer_bit_work_bound,
        )?,
        max_normalization_input_term_pairs: limits.max_normalization_input_term_pairs,
        max_guard_origins: limits.max_guard_origins,
        max_guard_origin_retained_bytes: limits.max_guard_origin_retained_bytes,
    })
}

fn residual_affine_guard_origin_copy_bytes<'a>(
    existing: impl IntoIterator<Item = &'a GuardOrigin>,
    added: impl IntoIterator<Item = &'a GuardOrigin>,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    existing
        .into_iter()
        .chain(added)
        .try_fold(0usize, |total, origin| {
            let bytes = origin.retained_byte_bound().ok_or(
                ResidualUnitAffineCompositionError::ResourceCountOverflow {
                    resource: "guard origin retained bytes",
                },
            )?;
            residual_affine_checked_add("guard origin retained bytes", total, bytes)
        })
}

fn merge_residual_affine_stats(
    left: ResidualUnitAffinePolynomialCompositionStats,
    right: ResidualUnitAffinePolynomialCompositionStats,
) -> Result<ResidualUnitAffinePolynomialCompositionStats, ResidualUnitAffineCompositionError> {
    Ok(ResidualUnitAffinePolynomialCompositionStats {
        source_terms: residual_affine_checked_add(
            "aggregate source terms",
            left.source_terms,
            right.source_terms,
        )?,
        source_exponent_entries: residual_affine_checked_add(
            "aggregate source exponent entries",
            left.source_exponent_entries,
            right.source_exponent_entries,
        )?,
        expanded_contribution_bound: residual_affine_checked_add(
            "aggregate expanded contributions",
            left.expanded_contribution_bound,
            right.expanded_contribution_bound,
        )?,
        output_terms: residual_affine_checked_add(
            "aggregate output terms",
            left.output_terms,
            right.output_terms,
        )?,
        output_exponent_entry_bound: residual_affine_checked_add(
            "aggregate prospective output exponent entries",
            left.output_exponent_entry_bound,
            right.output_exponent_entry_bound,
        )?,
        output_exponent_entries: residual_affine_checked_add(
            "aggregate output exponent entries",
            left.output_exponent_entries,
            right.output_exponent_entries,
        )?,
        power_calls: residual_affine_checked_add(
            "aggregate power calls",
            left.power_calls,
            right.power_calls,
        )?,
        native_power_heap_pair_bound: residual_affine_checked_add(
            "aggregate native power heap pairs",
            left.native_power_heap_pair_bound,
            right.native_power_heap_pair_bound,
        )?,
        multiplication_term_pair_bound: residual_affine_checked_add(
            "aggregate multiplication term pairs",
            left.multiplication_term_pair_bound,
            right.multiplication_term_pair_bound,
        )?,
        addition_term_visit_bound: residual_affine_checked_add(
            "aggregate addition term visits",
            left.addition_term_visit_bound,
            right.addition_term_visit_bound,
        )?,
        largest_kronecker_exponent_bits: left
            .largest_kronecker_exponent_bits
            .max(right.largest_kronecker_exponent_bits),
        largest_integer_coefficient_bit_bound: left
            .largest_integer_coefficient_bit_bound
            .max(right.largest_integer_coefficient_bit_bound),
        native_integer_bit_work_bound: residual_affine_checked_add(
            "aggregate native integer bit work",
            left.native_integer_bit_work_bound,
            right.native_integer_bit_work_bound,
        )?,
        integer_bit_work_bound: residual_affine_checked_add(
            "aggregate integer bit work",
            left.integer_bit_work_bound,
            right.integer_bit_work_bound,
        )?,
    })
}

fn residual_affine_affine_power_term_bound(
    exponent: usize,
    image_terms: usize,
    limit: usize,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    if image_terms == 0 {
        return Ok(0);
    }
    if exponent == 0 || image_terms == 1 {
        return Ok(1);
    }
    let n = exponent.checked_add(image_terms - 1).ok_or(
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "affine power terms",
        },
    )?;
    let k = exponent.min(image_terms - 1);
    let cap = limit as u128;
    let mut result = 1u128;
    for step in 1..=k {
        let mut numerator = (n - k + step) as u128;
        let mut denominator = step as u128;
        let common = residual_affine_u128_gcd(numerator, denominator);
        numerator /= common;
        denominator /= common;
        let common = residual_affine_u128_gcd(result, denominator);
        result /= common;
        denominator /= common;
        if denominator != 1 {
            return Err(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "affine power terms",
            });
        }
        if numerator != 0 && result > cap / numerator {
            if limit == usize::MAX {
                return Err(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                    resource: "affine power terms",
                });
            }
            return Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "affine power terms",
                requested: limit + 1,
                limit,
            });
        }
        result *= numerator;
    }
    usize::try_from(result).map_err(
        |_| ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "affine power terms",
        },
    )
}

/// Conservative structural-work census for Symbolica's Atom compositor.
///
/// `CoefficientPolynomial::to_expression` has at most one root plus two
/// nodes per source term and three nodes per dense source exponent slot.
/// Literal replacements are tested in order at every such node, hence the
/// `A * N_source` charge for the active replacement count A. RustRed scans all
/// R nonfree positions against all S source terms during structural census,
/// integer census, and execution, which contributes `3*R*S` probes.
/// Symbolica then
/// collects that iterator, so the census includes construction and
/// normalization of every active RHS. The remaining items cover the substituted tree,
/// every affine-power normalization, every prospective product, final
/// expansion/normalization and dense expanded-polynomial conversion. A
/// four-pass logical allowance at every level of the largest admitted shape
/// conservatively covers Symbolica's internal Atom ordering/traversal without
/// depending on an unstable standard-library sort comparison count.
fn residual_affine_symbolica_expression_structural_visit_bound(
    source_terms: usize,
    variable_count: usize,
    nonfree_replacement_count: usize,
    active_replacement_count: usize,
    replacement_image_terms: usize,
    replacement_occurrence_image_terms: usize,
    power_heap_pairs: usize,
    multiplication_term_pairs: usize,
    expanded_contributions: usize,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    let resource = "Symbolica expression structural term visits";
    let source_dense_slots = residual_affine_checked_mul(resource, source_terms, variable_count)?;
    let source_nodes = [
        1,
        residual_affine_checked_mul(resource, source_terms, 2)?,
        residual_affine_checked_mul(resource, source_dense_slots, 3)?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        residual_affine_checked_add(resource, sum, value)
    })?;
    let support_filter_probes = residual_affine_checked_mul(
        resource,
        residual_affine_checked_mul(resource, nonfree_replacement_count, source_terms)?,
        3,
    )?;
    let replacement_match_attempts =
        residual_affine_checked_mul(resource, active_replacement_count, source_nodes)?;

    let image_dense_slots =
        residual_affine_checked_mul(resource, replacement_image_terms, variable_count)?;
    let image_variable_sort_items =
        residual_affine_checked_mul(resource, active_replacement_count, variable_count)?;
    let image_nodes = residual_affine_checked_add(
        resource,
        active_replacement_count,
        residual_affine_checked_mul(resource, replacement_image_terms, 3)?,
    )?;
    let replacement_image_build = [image_dense_slots, image_variable_sort_items, image_nodes]
        .into_iter()
        .try_fold(0usize, |sum, value| {
            residual_affine_checked_add(resource, sum, value)
        })?;

    // One affine RHS with w terms contributes at most 1+3w Atom nodes.
    // The source root is already present, so every actual occurrence adds at
    // most 3w nodes after replacement.
    let substituted_nodes = residual_affine_checked_add(
        resource,
        source_nodes,
        residual_affine_checked_mul(resource, replacement_occurrence_image_terms, 3)?,
    )?;
    let twice_power_heap_pairs = residual_affine_checked_mul(resource, power_heap_pairs, 2)?;
    let multiplication_factor_visits =
        residual_affine_checked_mul(resource, multiplication_term_pairs, variable_count)?;
    let final_expression_terms = residual_affine_checked_mul(resource, expanded_contributions, 4)?;
    let conversion_exponent_slots =
        residual_affine_checked_mul(resource, expanded_contributions, variable_count)?;
    let ordered_items = [
        source_nodes,
        replacement_image_build,
        substituted_nodes,
        twice_power_heap_pairs,
        multiplication_factor_visits,
        final_expression_terms,
        residual_affine_checked_mul(resource, conversion_exponent_slots, 6)?,
    ]
    .into_iter()
    .try_fold(0usize, |sum, value| {
        residual_affine_checked_add(resource, sum, value)
    })?;
    let largest_shape = source_terms
        .max(expanded_contributions)
        .max(power_heap_pairs)
        .max(multiplication_term_pairs)
        .max(residual_affine_checked_add(
            resource,
            residual_affine_checked_mul(resource, variable_count, 2)?,
            2,
        )?)
        .max(2);
    let sort_factor = residual_affine_checked_mul(
        resource,
        residual_affine_checked_add(resource, residual_affine_ceil_log2(largest_shape), 1)?,
        4,
    )?;
    let ordering_visits = residual_affine_checked_mul(resource, ordered_items, sort_factor)?;
    residual_affine_checked_add(
        resource,
        support_filter_probes,
        residual_affine_checked_add(resource, replacement_match_attempts, ordering_visits)?,
    )
}

/// Conservative integer-work census for Symbolica's Atom expansion backend.
///
/// For every one of the `H(e,w)` materialized power monomials,
/// `Integer::multinom` performs at most `e` binomial iterations (two integer
/// multiply/divide operations each), followed by coefficient powers and
/// products for the `w` image terms. The logarithmic factor covers the
/// integer sizes used to represent multiplicities and binomial intermediates.
/// This is resource arithmetic only; all polynomial algebra remains inside
/// Symbolica.
fn residual_affine_symbolica_expression_integer_preflight(
    source: &CoefficientPolynomial,
    plan: &ResidualAffineCompositionCorePlan,
    base_count: usize,
    power_term_limit: usize,
) -> Result<ResidualAffineSymbolicaExpressionIntegerPreflight, ResidualUnitAffineCompositionError> {
    let mut largest_integer_coefficient_bit_bound = 0usize;
    let mut largest_integer_contribution_bit_bound = 0usize;
    let mut integer_bit_work_bound = 0usize;

    // Every active affine RHS is converted from a Symbolica polynomial to an
    // Atom before simultaneous replacement. `to_expression` makes two
    // magnitude copies per coefficient (the polynomial clone and serialized
    // numerator); inactive images are filtered and incur no RHS construction.
    for &position in &plan.nonfree_positions {
        let variable = base_count.checked_add(position).ok_or(
            ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "Symbolica expression replacement image integers",
            },
        )?;
        let occurrence_count =
            source
                .exponents_iter()
                .try_fold(0usize, |occurrences, exponents| {
                    if exponents[variable] == 0 {
                        Ok(occurrences)
                    } else {
                        residual_affine_checked_add(
                            "Symbolica expression replacement occurrences",
                            occurrences,
                            1,
                        )
                    }
                })?;
        if occurrence_count == 0 {
            continue;
        }
        for coefficient in &plan.full_images[variable].coefficients {
            let coefficient_bits = residual_affine_integer_bits(coefficient)?;
            largest_integer_coefficient_bit_bound =
                largest_integer_coefficient_bit_bound.max(coefficient_bits);
            integer_bit_work_bound = residual_affine_checked_add(
                "Symbolica expression integer bit work",
                integer_bit_work_bound,
                residual_affine_checked_mul(
                    "Symbolica expression replacement integer bit work",
                    2,
                    coefficient_bits,
                )?,
            )?;
        }
    }

    for source_term in 0..source.nterms() {
        let source_integer_bits = residual_affine_integer_bits(&source.coefficients[source_term])?;
        largest_integer_coefficient_bit_bound =
            largest_integer_coefficient_bit_bound.max(source_integer_bits);
        integer_bit_work_bound = residual_affine_checked_add(
            "Symbolica expression integer bit work",
            integer_bit_work_bound,
            residual_affine_checked_mul(
                "Symbolica expression source integer bit work",
                2,
                source_integer_bits,
            )?,
        )?;

        let mut contribution_bound = 1usize;
        let mut contribution_integer_bits = source_integer_bits;
        for (variable, &exponent) in source.exponents(source_term).iter().enumerate() {
            if exponent == 0 {
                continue;
            }
            let exponent = usize::from(exponent);
            let image_terms = plan.image_term_counts[variable];
            let power_terms =
                residual_affine_affine_power_term_bound(exponent, image_terms, power_term_limit)?;
            let multiplication_pairs = residual_affine_checked_mul(
                "Symbolica expression multiplication term pairs",
                contribution_bound,
                power_terms,
            )?;

            if power_terms != 0 {
                let per_power_growth = plan.image_coefficient_growth_bits[variable]
                    .checked_add(residual_affine_ceil_log2(image_terms))
                    .and_then(|bits| bits.checked_mul(exponent))
                    .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                        resource: "Symbolica expression integer coefficient growth bits",
                    })?;
                let power_final_integer_bits = per_power_growth.checked_add(1).ok_or(
                    ResidualUnitAffineCompositionError::ResourceCountOverflow {
                        resource: "Symbolica expression integer coefficient bits",
                    },
                )?;
                let exponent_log = residual_affine_ceil_log2(residual_affine_checked_add(
                    "Symbolica expression power work units",
                    exponent,
                    1,
                )?);
                let per_term_work_units = [
                    residual_affine_checked_mul(
                        "Symbolica expression power work units",
                        exponent,
                        2,
                    )?,
                    residual_affine_checked_mul(
                        "Symbolica expression power work units",
                        image_terms,
                        2,
                    )?,
                    residual_affine_checked_mul(
                        "Symbolica expression power work units",
                        residual_affine_checked_mul(
                            "Symbolica expression power work units",
                            image_terms,
                            exponent_log,
                        )?,
                        2,
                    )?,
                    4,
                ]
                .into_iter()
                .try_fold(0usize, |sum, units| {
                    residual_affine_checked_add("Symbolica expression power work units", sum, units)
                })?;
                let power_work_units = residual_affine_checked_mul(
                    "Symbolica expression power work units",
                    power_terms,
                    per_term_work_units,
                )?;
                let power_transient_integer_bits = residual_affine_checked_add(
                    "Symbolica expression transient integer bits",
                    power_final_integer_bits,
                    exponent_log,
                )?;
                largest_integer_coefficient_bit_bound =
                    largest_integer_coefficient_bit_bound.max(power_transient_integer_bits);
                let power_integer_bit_work = residual_affine_checked_mul(
                    "Symbolica expression integer bit work",
                    power_work_units,
                    power_transient_integer_bits,
                )?;
                integer_bit_work_bound = residual_affine_checked_add(
                    "Symbolica expression integer bit work",
                    integer_bit_work_bound,
                    power_integer_bit_work,
                )?;

                contribution_integer_bits = residual_affine_checked_add(
                    "Symbolica expression contribution integer bits",
                    contribution_integer_bits,
                    power_final_integer_bits,
                )?;
                if multiplication_pairs != 0 {
                    largest_integer_coefficient_bit_bound =
                        largest_integer_coefficient_bit_bound.max(contribution_integer_bits);
                }
                let multiplication_integer_bit_work = residual_affine_checked_mul(
                    "Symbolica expression integer bit work",
                    multiplication_pairs,
                    contribution_integer_bits,
                )?;
                integer_bit_work_bound = residual_affine_checked_add(
                    "Symbolica expression integer bit work",
                    integer_bit_work_bound,
                    multiplication_integer_bit_work,
                )?;
            }
            contribution_bound = multiplication_pairs;
        }
        if contribution_bound != 0 {
            largest_integer_contribution_bit_bound =
                largest_integer_contribution_bit_bound.max(contribution_integer_bits);
        }
    }

    Ok(ResidualAffineSymbolicaExpressionIntegerPreflight {
        largest_integer_coefficient_bit_bound,
        largest_integer_contribution_bit_bound,
        integer_bit_work_bound,
    })
}

fn residual_affine_u128_gcd(mut left: u128, mut right: u128) -> u128 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ResidualAffineKroneckerPreflight {
    exponent_bits: usize,
    polynomial_evaluator_safe: bool,
}

fn residual_affine_kronecker_preflight(
    plan: &ResidualAffineCompositionCorePlan,
    source_variable: usize,
    exponent: usize,
    base_count: usize,
) -> Result<ResidualAffineKroneckerPreflight, ResidualUnitAffineCompositionError> {
    let image_terms = plan.image_term_counts[source_variable];
    if exponent <= 1 || image_terms <= 1 {
        return Ok(ResidualAffineKroneckerPreflight {
            exponent_bits: 0,
            polynomial_evaluator_safe: true,
        });
    }
    let degree_variable_count = if source_variable < base_count {
        1
    } else {
        let position = source_variable - base_count;
        let mut count = 0usize;
        for free_ordinal in 0..plan.free_positions.len() {
            if plan.linear_is_nonzero(position, free_ordinal).ok_or(
                ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                    resource: "affine linear support",
                    actual: plan.linear_support.len(),
                    bound: plan.ambient_arity.saturating_mul(plan.free_positions.len()),
                },
            )? {
                count = residual_affine_checked_add("Kronecker degree variables", count, 1)?;
            }
        }
        count
    };
    let stride_radix_variable_count = if source_variable < base_count {
        usize::from(source_variable != 0)
    } else {
        let position = source_variable - base_count;
        let mut count = 0usize;
        for free_ordinal in 0..plan.free_positions.len() {
            let target_variable = base_count + plan.free_positions[free_ordinal];
            if target_variable != 0
                && plan.linear_is_nonzero(position, free_ordinal).ok_or(
                    ResidualUnitAffineCompositionError::CompositionInvariantViolation {
                        resource: "affine linear support",
                        actual: plan.linear_support.len(),
                        bound: plan.ambient_arity.saturating_mul(plan.free_positions.len()),
                    },
                )?
            {
                count = residual_affine_checked_add("Kronecker stride variables", count, 1)?;
            }
        }
        count
    };
    let radix_factor = exponent.checked_add(1).ok_or(
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "Kronecker radix",
        },
    )?;
    let exponent_bits = residual_affine_checked_mul(
        "Kronecker exponent bits",
        degree_variable_count,
        residual_affine_ceil_log2(radix_factor),
    )?;

    // In the audited vendored revision `heap_pow` infers its unannotated
    // running stride as u32 even though the encoded exponent is promoted to
    // `Integer` afterwards. Crossing this backend representation ceiling
    // selects Symbolica's expression-expansion compositor; it is not itself
    // a caller-visible resource rejection. The independently configured
    // exponent-bit limit is enforced above for both Symbolica backends.
    let mut exact_stride = 1u64;
    let mut polynomial_evaluator_safe = true;
    for _ in 0..stride_radix_variable_count {
        let Some(next) = exact_stride.checked_mul(radix_factor as u64) else {
            polynomial_evaluator_safe = false;
            break;
        };
        if next > u64::from(u32::MAX) {
            polynomial_evaluator_safe = false;
            break;
        }
        exact_stride = next;
    }
    Ok(ResidualAffineKroneckerPreflight {
        exponent_bits,
        polynomial_evaluator_safe,
    })
}

fn check_residual_affine_exponent(
    source_term: usize,
    target_variable: usize,
    requested: u128,
    limit: u128,
) -> Result<(), ResidualUnitAffineCompositionError> {
    if requested > limit {
        Err(ResidualUnitAffineCompositionError::ExponentLimit {
            source_term,
            target_variable,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn residual_affine_integer_bits(
    value: &Integer,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    usize::try_from(integer_magnitude_bits(value)).map_err(|_| {
        ResidualUnitAffineCompositionError::ResourceCountOverflow {
            resource: "integer coefficient bits",
        }
    })
}

fn residual_affine_integer_growth_bits(
    value: &Integer,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    let magnitude_bits = residual_affine_integer_bits(value)?;
    if magnitude_bits <= 1 {
        return Ok(0);
    }
    // bits(|x|-1) is one below bits(|x|) exactly when |x| is a power
    // of two. Rug's borrowed absolute view is allocation-free, so plan replay
    // never creates a GMP predecessor scratch merely to authenticate growth
    // metadata.
    let magnitude_is_power_of_two = match value {
        Integer::Single(value) => value.unsigned_abs().is_power_of_two(),
        Integer::Double(value) => value.unsigned_abs().is_power_of_two(),
        Integer::Large(value) => value.as_abs().is_power_of_two(),
    };
    Ok(magnitude_bits - usize::from(magnitude_is_power_of_two))
}

fn residual_affine_ceil_log2(value: usize) -> usize {
    if value <= 1 {
        0
    } else {
        usize::BITS as usize - (value - 1).leading_zeros() as usize
    }
}

fn residual_affine_checked_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    left.checked_add(right)
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })
}

fn residual_affine_checked_mul(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ResidualUnitAffineCompositionError> {
    left.checked_mul(right)
        .ok_or(ResidualUnitAffineCompositionError::ResourceCountOverflow { resource })
}

fn check_residual_affine_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ResidualUnitAffineCompositionError> {
    if requested > limit {
        Err(ResidualUnitAffineCompositionError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn specialization_integer_bit_bound(
    coefficient: &Integer,
    exponents: &[u16],
    base_count: usize,
    assignment: &[i64],
) -> Result<usize, ParametricCoefficientError> {
    let mut requested = integer_magnitude_bits(coefficient);
    if requested == 0 {
        return Ok(0);
    }
    for (position, value) in assignment.iter().copied().enumerate() {
        let exponent = exponents[base_count + position];
        if exponent == 0 {
            continue;
        }
        let magnitude = value.unsigned_abs();
        if magnitude == 0 {
            return Ok(0);
        }
        // Multiplication by (+/-1)^e does not grow the coefficient.  For all
        // other bases, e*bit_length(base) is a conservative bit bound for the
        // power and hence for its contribution to the product.
        if magnitude != 1 {
            let value_bits = u128::from(u64::BITS - magnitude.leading_zeros());
            let power_bits = value_bits.checked_mul(u128::from(exponent)).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "coefficient specialization integer bits",
                },
            )?;
            requested = requested.checked_add(power_bits).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "coefficient specialization integer bits",
                },
            )?;
        }
    }
    usize::try_from(requested).map_err(|_| ParametricCoefficientError::ResourceCountOverflow {
        resource: "coefficient specialization integer bits",
    })
}

fn partial_specialization_integer_bit_bound(
    coefficient: &Integer,
    exponents: &[u16],
    base_count: usize,
    assignment: &[(usize, i64)],
) -> Result<usize, ParametricCoefficientError> {
    let mut requested = integer_magnitude_bits(coefficient);
    if requested == 0 {
        return Ok(0);
    }
    for &(position, value) in assignment {
        let exponent = exponents[base_count + position];
        if exponent == 0 {
            continue;
        }
        let magnitude = value.unsigned_abs();
        if magnitude == 0 {
            return Ok(0);
        }
        if magnitude != 1 {
            let value_bits = u128::from(u64::BITS - magnitude.leading_zeros());
            let power_bits = value_bits.checked_mul(u128::from(exponent)).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "partial polynomial specialization integer bits",
                },
            )?;
            requested = requested.checked_add(power_bits).ok_or(
                ParametricCoefficientError::ResourceCountOverflow {
                    resource: "partial polynomial specialization integer bits",
                },
            )?;
        }
    }
    usize::try_from(requested).map_err(|_| ParametricCoefficientError::ResourceCountOverflow {
        resource: "partial polynomial specialization integer bits",
    })
}

fn checked_partial_stat_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, ParametricCoefficientError> {
    left.checked_add(right)
        .ok_or(ParametricCoefficientError::ResourceCountOverflow { resource })
}

fn integer_magnitude_bits(value: &Integer) -> u128 {
    match value {
        Integer::Single(value) => {
            let magnitude = value.unsigned_abs();
            u128::from(i64::BITS - magnitude.leading_zeros())
        }
        Integer::Double(value) => {
            let magnitude = value.unsigned_abs();
            u128::from(i128::BITS - magnitude.leading_zeros())
        }
        Integer::Large(value) => u128::from(value.significant_bits()),
    }
}

pub(crate) fn insert_parametric_condition(
    conditions: &mut Vec<ParametricNonZeroCondition>,
    condition: ParametricNonZeroCondition,
    max_guard_origins: usize,
) -> Result<(), ParametricCoefficientError> {
    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial == condition.polynomial)
    {
        existing.merge_origins_from(&condition, max_guard_origins)
    } else {
        check_limit(
            "parametric guard origins",
            condition.origins.len(),
            max_guard_origins,
        )?;
        conditions.push(condition);
        Ok(())
    }
}

pub(crate) fn insert_specialized_condition(
    conditions: &mut Vec<SpecializedNonZeroCondition>,
    condition: SpecializedNonZeroCondition,
    max_guard_origins: usize,
) -> Result<(), ParametricCoefficientError> {
    if let Some(existing) = conditions
        .iter_mut()
        .find(|existing| existing.polynomial == condition.polynomial)
    {
        existing.merge_origins_from(&condition, max_guard_origins)
    } else {
        check_limit(
            "specialized guard origins",
            condition.origins.len(),
            max_guard_origins,
        )?;
        conditions.push(condition);
        Ok(())
    }
}

fn collect_guard_origins_with_limit(
    origins: impl IntoIterator<Item = GuardOrigin>,
    max_guard_origins: usize,
) -> Result<BTreeSet<GuardOrigin>, ParametricCoefficientError> {
    let mut collected = BTreeSet::new();
    for (position, origin) in origins.into_iter().enumerate() {
        let requested =
            position
                .checked_add(1)
                .ok_or(ParametricCoefficientError::ResourceCountOverflow {
                    resource: "parametric guard origin inputs",
                })?;
        check_limit(
            "parametric guard origin inputs",
            requested,
            max_guard_origins,
        )?;
        collected.insert(origin);
    }
    check_limit(
        "parametric guard origins",
        collected.len(),
        max_guard_origins,
    )?;
    Ok(collected)
}

fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ParametricCoefficientError> {
    if requested > limit {
        Err(ParametricCoefficientError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn encode_symbol_component(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

fn base_context_fingerprint(base: &CoefficientContext) -> String {
    let mut result = format!(
        "rustred-base-context-v1|parameters={}",
        base.parameter_names().len()
    );
    for name in base.parameter_names() {
        result.push('|');
        result.push_str(&name.len().to_string());
        result.push(':');
        result.push_str(name);
    }
    result
}

#[cfg(test)]
mod tests {
    use symbolica::domains::integer::MultiPrecisionInteger;

    use super::*;
    use crate::{
        CoordinateEqualityLocusExtractor, CoordinateEqualityLocusLimits, GuardRowId,
        ResidualAffineIntegerSystemInputRow, ResidualAffineIntegerSystemLimits,
        ResidualAffinePrimitiveRow, ResidualUnitAffineIndexMapLimits, SectorMask,
        SymbolicPolynomialPredicateKind, SymbolicSectorCaseLimits,
        SymbolicSectorCasePartitionBuilder,
    };

    fn residual_affine_integer_system_row(
        components: Vec<Integer>,
        structural_locus_ordinal: usize,
    ) -> ResidualAffineIntegerSystemInputRow {
        let component_count = components.len();
        let row = ResidualAffinePrimitiveRow::try_from_canonical_components_with_limits(
            components,
            component_count,
            1_000_000,
            100_000_000,
        )
        .unwrap();
        ResidualAffineIntegerSystemInputRow::try_new(row, vec![structural_locus_ordinal], 8)
            .unwrap()
    }

    fn residual_affine_integer_system_certificate(
        arity: usize,
        rows: Vec<ResidualAffineIntegerSystemInputRow>,
    ) -> Arc<ResidualAffineIntegerSystemCertificate> {
        Arc::new(
            ResidualAffineIntegerSystemCertificate::compile(
                arity,
                &rows,
                ResidualAffineIntegerSystemLimits::default(),
            )
            .unwrap(),
        )
    }

    fn residual_affine_test_map(
        context: &ParametricCoefficientContext,
        include_literal_n2: bool,
    ) -> Arc<ResidualUnitAffineIndexMapCertificate> {
        let affine = context
            .sub(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.integer(3),
            )
            .unwrap();
        residual_affine_test_map_for_predicate(context, affine, include_literal_n2)
    }

    fn residual_affine_test_map_for_predicate(
        context: &ParametricCoefficientContext,
        affine: ParametricCoefficient,
        include_literal_n2: bool,
    ) -> Arc<ResidualUnitAffineIndexMapCertificate> {
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            context,
            SectorMask::try_new([true, true, true]).unwrap(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        let mut leaf = builder.root_case();
        if include_literal_n2 {
            let literal = context
                .sub(&context.index(2).unwrap(), &context.integer(2))
                .unwrap();
            leaf = builder
                .split_on_bad_polynomial(
                    context,
                    leaf,
                    context.numerator_condition(&literal).unwrap(),
                )
                .unwrap()
                .equal_zero_case();
        }
        leaf = builder
            .split_on_bad_polynomial(context, leaf, context.numerator_condition(&affine).unwrap())
            .unwrap()
            .equal_zero_case();
        let partition = builder.finish(context).unwrap();
        let source = Arc::new(
            CoordinateEqualityLocusExtractor::extract(
                context,
                &partition,
                leaf,
                CoordinateEqualityLocusLimits::default(),
            )
            .unwrap(),
        );
        let predicate_ordinal = source
            .unresolved_predicates()
            .iter()
            .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
            .unwrap()
            .predicate_ordinal();
        Arc::new(
            ResidualUnitAffineIndexMapCertificate::compile(
                context,
                source,
                predicate_ordinal,
                0,
                ResidualUnitAffineIndexMapLimits::default(),
            )
            .unwrap(),
        )
    }

    fn residual_affine_test_context(scope: &str) -> ParametricCoefficientContext {
        ParametricCoefficientContext::try_new(&CoefficientContext::new(["d", "m2"]), scope, 3)
            .unwrap()
    }

    #[test]
    fn authenticated_rational_sparse_payload_copy_is_deep_and_context_shared() {
        let context = residual_affine_test_context("authenticated-rational-deep-copy");
        let numerator = context
            .add(&context.index(0).unwrap(), &context.integer(2))
            .unwrap();
        let denominator = context
            .sub(&context.index(1).unwrap(), &context.integer(3))
            .unwrap();
        let source = context.checked_div(&numerator, &denominator).unwrap();
        let copy = source.try_copy_authenticated_sparse_payload().unwrap();

        assert_eq!(copy, source);
        assert!(context.contains(&copy));
        assert_eq!(
            copy.owned_retained_byte_bound(),
            source.owned_retained_byte_bound()
        );
        assert!(Arc::ptr_eq(&source.context, &copy.context));
        assert!(Arc::ptr_eq(
            &source.raw.numerator.variables,
            &copy.raw.numerator.variables
        ));
        assert!(Arc::ptr_eq(
            &source.raw.denominator.variables,
            &copy.raw.denominator.variables
        ));
        assert_ne!(
            source.raw.numerator.coefficients.as_ptr(),
            copy.raw.numerator.coefficients.as_ptr()
        );
        assert_ne!(
            source.raw.numerator.exponents.as_ptr(),
            copy.raw.numerator.exponents.as_ptr()
        );
        assert_ne!(
            source.raw.denominator.coefficients.as_ptr(),
            copy.raw.denominator.coefficients.as_ptr()
        );
        assert_ne!(
            source.raw.denominator.exponents.as_ptr(),
            copy.raw.denominator.exponents.as_ptr()
        );
    }

    fn residual_affine_all_pivot_zero_legacy_map(
        context: &ParametricCoefficientContext,
    ) -> Arc<ResidualUnitAffineIndexMapCertificate> {
        assert_eq!(context.index_count(), 3);
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            context,
            SectorMask::try_new([false, false, false]).unwrap(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        let mut leaf = builder.root_case();
        for position in [1, 2] {
            leaf = builder
                .split_on_bad_polynomial(
                    context,
                    leaf,
                    context
                        .numerator_condition(&context.index(position).unwrap())
                        .unwrap(),
                )
                .unwrap()
                .equal_zero_case();
        }
        let predicate = context
            .add(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.index(2).unwrap(),
            )
            .unwrap();
        leaf = builder
            .split_on_bad_polynomial(
                context,
                leaf,
                context.numerator_condition(&predicate).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
        let partition = builder.finish(context).unwrap();
        let source = Arc::new(
            CoordinateEqualityLocusExtractor::extract(
                context,
                &partition,
                leaf,
                CoordinateEqualityLocusLimits::default(),
            )
            .unwrap(),
        );
        let predicate_ordinal = source
            .unresolved_predicates()
            .iter()
            .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
            .unwrap()
            .predicate_ordinal();
        let map = Arc::new(
            ResidualUnitAffineIndexMapCertificate::compile(
                context,
                source,
                predicate_ordinal,
                0,
                ResidualUnitAffineIndexMapLimits::default(),
            )
            .unwrap(),
        );
        assert!(map.free_positions().is_empty());
        assert!(
            (0..context.index_count())
                .all(|position| map.constant(position).is_some_and(Integer::is_zero))
        );
        map
    }

    fn residual_affine_plan(
        context: &ParametricCoefficientContext,
        include_literal_n2: bool,
    ) -> ResidualUnitAffineCompositionPlan {
        context
            .compile_residual_unit_affine_composition_plan(
                residual_affine_test_map(context, include_literal_n2),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap()
    }

    fn residual_affine_integer_plan(
        context: &ParametricCoefficientContext,
    ) -> ResidualAffineCompositionPlan {
        assert_eq!(context.index_count(), 3);
        // 3 - n_0 - n_1 = 0, hence F(n_1,n_2) = (3-n_1,n_1,n_2).
        let certificate = residual_affine_integer_system_certificate(
            3,
            vec![residual_affine_integer_system_row(
                vec![3, -1, -1, 0].into_iter().map(Integer::from).collect(),
                0,
            )],
        );
        context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap()
    }

    fn residual_affine_compact_test_plan(
        context: &ParametricCoefficientContext,
    ) -> ResidualAffineCompactCompositionPlan {
        assert_eq!(context.index_count(), 3);
        // F(n_1,n_2) = (2+n_1-n_2,n_1,n_2).
        let constants = [Integer::from(2), Integer::zero(), Integer::zero()];
        let free_positions = [1usize, 2];
        let compact_linear_coefficients = [
            Integer::one(),
            Integer::from(-1),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        context
            .compile_residual_affine_compact_composition_plan(
                ResidualAffineCompactMapView::new(
                    context.fingerprint(),
                    3,
                    &constants,
                    &free_positions,
                    &compact_linear_coefficients,
                ),
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap()
    }

    fn residual_affine_zero_n0_bound_n2_plan(
        context: &ParametricCoefficientContext,
    ) -> ResidualUnitAffineCompositionPlan {
        let mut builder = SymbolicSectorCasePartitionBuilder::try_new(
            context,
            SectorMask::try_new([false, true, true]).unwrap(),
            SymbolicSectorCaseLimits::default(),
        )
        .unwrap();
        let root = builder.root_case();
        let literal_leaf = builder
            .split_on_bad_polynomial(
                context,
                root,
                context
                    .numerator_condition(&context.index(0).unwrap())
                    .unwrap(),
            )
            .unwrap()
            .equal_zero_case();
        let affine = context
            .sub(
                &context
                    .add(&context.index(2).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.integer(3),
            )
            .unwrap();
        let leaf = builder
            .split_on_bad_polynomial(
                context,
                literal_leaf,
                context.numerator_condition(&affine).unwrap(),
            )
            .unwrap()
            .equal_zero_case();
        let partition = builder.finish(context).unwrap();
        let source = Arc::new(
            CoordinateEqualityLocusExtractor::extract(
                context,
                &partition,
                leaf,
                CoordinateEqualityLocusLimits::default(),
            )
            .unwrap(),
        );
        let predicate_ordinal = source
            .unresolved_predicates()
            .iter()
            .find(|predicate| predicate.kind() == SymbolicPolynomialPredicateKind::EqualZero)
            .unwrap()
            .predicate_ordinal();
        let map = Arc::new(
            ResidualUnitAffineIndexMapCertificate::compile(
                context,
                source,
                predicate_ordinal,
                2,
                ResidualUnitAffineIndexMapLimits::default(),
            )
            .unwrap(),
        );
        context
            .compile_residual_unit_affine_composition_plan(
                map,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap()
    }

    fn residual_affine_polynomial(
        context: &ParametricCoefficientContext,
        value: &ParametricCoefficient,
    ) -> ParametricPolynomial {
        context.numerator_condition(value).unwrap()
    }

    #[test]
    fn compact_affine_v2_prepared_execution_is_native_and_preflights_once() {
        let context = residual_affine_test_context("compact-affine-v2-prepared-native");
        let plan = residual_affine_compact_test_plan(&context);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let image = context
            .sub(&context.add(&context.integer(2), &n1).unwrap(), &n2)
            .unwrap();
        let guard_value = context.add(&context.mul(&n0, &n1).unwrap(), &n2).unwrap();
        let guard = residual_affine_polynomial(&context, &guard_value);
        let denominator = context
            .sub(&context.add(&n0, &n2).unwrap(), &context.one())
            .unwrap();
        let coefficient = context.checked_div(&guard_value, &denominator).unwrap();
        let expected_guard = context
            .add(&context.mul(&image, &n1).unwrap(), &n2)
            .unwrap();
        let expected_denominator = context.add(&context.one(), &n1).unwrap();
        let expected_coefficient = context
            .checked_div(&expected_guard, &expected_denominator)
            .unwrap();
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();

        reset_residual_affine_compact_preflight_calls_for_test();
        let prepared_guard = context
            .prepare_guard_on_residual_affine_compact_composition_plan(&guard, &plan, limits)
            .unwrap();
        let guard_preflight = prepared_guard.stats();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 1);
        let mapped_guard = prepared_guard.execute().unwrap();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 1);
        assert_eq!(
            mapped_guard.value(),
            &residual_affine_polynomial(&context, &expected_guard)
        );
        assert_eq!(
            mapped_guard.stats().source_terms(),
            guard_preflight.source_terms()
        );
        assert!(
            mapped_guard.stats().output_terms() <= guard_preflight.expanded_contribution_bound()
        );

        // Independent native Symbolica full-point evaluation oracle.
        let ring = PolynomialRing::<IntegerRing, u16>::from_poly(&context.template.numerator);
        let direct = guard.raw.evaluate_with_coeff_map(
            |integer| context.template.numerator.constant(integer.clone()),
            &plan.core.full_images,
            &ring,
        );
        assert_eq!(mapped_guard.value().raw, direct);

        reset_residual_affine_compact_preflight_calls_for_test();
        let prepared_coefficient = context
            .prepare_coefficient_on_residual_affine_compact_composition_plan(
                &coefficient,
                &plan,
                limits,
            )
            .unwrap();
        let coefficient_preflight = prepared_coefficient.stats();
        // One preflight per rational half, both retained by the single token.
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
        let mapped_coefficient = prepared_coefficient.execute().unwrap();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
        let ResidualAffineCoefficientComposition::Available(mapped_coefficient) =
            mapped_coefficient
        else {
            panic!("the prepared denominator 1+n1 is generically nonzero")
        };
        assert_eq!(mapped_coefficient.value(), &expected_coefficient);
        assert_eq!(
            mapped_coefficient.mapped_denominator(),
            &residual_affine_polynomial(&context, &expected_denominator)
        );
        assert_eq!(
            mapped_coefficient.stats().numerator().source_terms(),
            coefficient_preflight.numerator().source_terms()
        );
        assert_eq!(
            mapped_coefficient.stats().denominator().source_terms(),
            coefficient_preflight.denominator().source_terms()
        );
        assert!(
            mapped_coefficient.stats().normalization_input_term_pairs()
                <= coefficient_preflight.normalization_input_term_pair_bound()
        );
    }

    #[test]
    fn compact_affine_v2_prepared_coefficient_preserves_zero_denominator() {
        let context = residual_affine_test_context("compact-affine-v2-prepared-zero-denominator");
        let plan = residual_affine_compact_test_plan(&context);
        let equality = context
            .add(
                &context
                    .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context
                    .sub(&context.index(2).unwrap(), &context.integer(2))
                    .unwrap(),
            )
            .unwrap();
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: context.template.numerator.one(),
                denominator: equality.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();

        reset_residual_affine_compact_preflight_calls_for_test();
        let prepared = context
            .prepare_coefficient_on_residual_affine_compact_composition_plan(&source, &plan, limits)
            .unwrap();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
        let mapped = prepared.execute().unwrap();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
        assert!(matches!(
            mapped,
            ResidualAffineCoefficientComposition::ZeroMappedDenominator { .. }
        ));
        assert_eq!(mapped.stats().denominator().output_terms(), 0);
        assert_eq!(mapped.stats().durable_denominator_terms(), 0);
        assert_eq!(mapped.stats().durable_guard_origin_retained_bytes(), 0);
    }

    #[test]
    fn compact_affine_v2_prepared_coefficient_preserves_cancelled_denominator() {
        let context = residual_affine_test_context("compact-affine-v2-prepared-cancellation");
        let plan = residual_affine_compact_test_plan(&context);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        // These are distinct in the ambient ring, but both map to 1+n1.
        let source_numerator = context
            .sub(&context.add(&n0, &n2).unwrap(), &context.one())
            .unwrap();
        let source_denominator = context.add(&n1, &context.one()).unwrap();
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: source_numerator.raw.numerator.clone(),
                denominator: source_denominator.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();

        reset_residual_affine_compact_preflight_calls_for_test();
        let prepared = context
            .prepare_coefficient_on_residual_affine_compact_composition_plan(&source, &plan, limits)
            .unwrap();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
        let mapped = prepared.execute().unwrap();
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
        let ResidualAffineCoefficientComposition::Available(mapped) = mapped else {
            panic!("the mapped denominator 1+n1 is generically nonzero")
        };
        assert_eq!(mapped.value(), &context.one());
        assert_eq!(
            mapped.mapped_denominator(),
            &residual_affine_polynomial(&context, &source_denominator)
        );
        assert!(mapped.stats().durable_denominator_terms() > 0);
        assert_eq!(mapped.stats().durable_guard_origin_retained_bytes(), 0);
    }

    #[test]
    fn compact_affine_v2_prepared_execution_fails_during_bounded_preflight() {
        let context = residual_affine_test_context("compact-affine-v2-prepared-limits");
        let plan = residual_affine_compact_test_plan(&context);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let guard_value = context.add(&n0, &n1).unwrap();
        let guard = residual_affine_polynomial(&context, &guard_value);

        let mut guard_limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        guard_limits.max_source_terms = 0;
        reset_residual_affine_compact_preflight_calls_for_test();
        assert!(matches!(
            context.prepare_guard_on_residual_affine_compact_composition_plan(
                &guard,
                &plan,
                guard_limits,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "polynomial source terms",
                limit: 0,
                ..
            })
        ));
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 1);

        let numerator = context.add(&n0, &context.one()).unwrap();
        let denominator = context.add(&n1, &context.one()).unwrap();
        let coefficient = context.checked_div(&numerator, &denominator).unwrap();
        let mut coefficient_limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        coefficient_limits.max_normalization_input_term_pairs = 0;
        reset_residual_affine_compact_preflight_calls_for_test();
        assert!(matches!(
            context.prepare_coefficient_on_residual_affine_compact_composition_plan(
                &coefficient,
                &plan,
                coefficient_limits,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "coefficient normalization input term-pair bound",
                limit: 0,
                ..
            })
        ));
        assert_eq!(residual_affine_compact_preflight_calls_for_test(), 2);
    }

    #[test]
    fn compact_affine_v2_composes_translated_rational_and_guard_differentially() {
        let context = residual_affine_test_context("compact-affine-v2-rational-guard");
        let plan = residual_affine_compact_test_plan(&context);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let image = context
            .sub(&context.add(&context.integer(2), &n1).unwrap(), &n2)
            .unwrap();

        let numerator = context
            .add(
                &context
                    .add(
                        &context.mul(&n0, &n0).unwrap(),
                        &context.mul(&n1, &n2).unwrap(),
                    )
                    .unwrap(),
                &context.add(&n0, &context.integer(7)).unwrap(),
            )
            .unwrap();
        let denominator = context
            .sub(&context.add(&n0, &n2).unwrap(), &context.one())
            .unwrap();
        let source = context.checked_div(&numerator, &denominator).unwrap();
        let expected_numerator = context
            .add(
                &context
                    .add(
                        &context.mul(&image, &image).unwrap(),
                        &context.mul(&n1, &n2).unwrap(),
                    )
                    .unwrap(),
                &context.add(&image, &context.integer(7)).unwrap(),
            )
            .unwrap();
        let expected_denominator = context.add(&context.one(), &n1).unwrap();
        let expected = context
            .checked_div(&expected_numerator, &expected_denominator)
            .unwrap();
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let preflight = context
            .preflight_coefficient_on_residual_affine_compact_composition_plan(
                &source, &plan, limits,
            )
            .unwrap();
        let ResidualAffineCoefficientComposition::Available(mapped) = context
            .compose_coefficient_on_residual_affine_compact_composition_plan(&source, &plan, limits)
            .unwrap()
        else {
            panic!("the translated denominator 1+n1 is generically nonzero")
        };
        assert_eq!(mapped.value(), &expected);
        assert_eq!(
            mapped.mapped_denominator(),
            &residual_affine_polynomial(&context, &expected_denominator)
        );
        assert_eq!(mapped.stats().durable_guard_origin_retained_bytes(), 0);
        assert!(
            preflight.normalization_input_term_pair_bound()
                >= mapped.stats().normalization_input_term_pairs()
        );

        let guard_value = context
            .add(
                &context
                    .add(
                        &context.mul(&n0, &n1).unwrap(),
                        &context.mul(&n0, &n2).unwrap(),
                    )
                    .unwrap(),
                &context
                    .add(&context.mul(&n1, &n2).unwrap(), &context.integer(3))
                    .unwrap(),
            )
            .unwrap();
        let guard = residual_affine_polynomial(&context, &guard_value);
        let expected_guard = context
            .add(
                &context
                    .add(
                        &context.mul(&image, &n1).unwrap(),
                        &context.mul(&image, &n2).unwrap(),
                    )
                    .unwrap(),
                &context
                    .add(&context.mul(&n1, &n2).unwrap(), &context.integer(3))
                    .unwrap(),
            )
            .unwrap();
        let native = context
            .compose_guard_on_residual_affine_compact_composition_plan(&guard, &plan, limits)
            .unwrap();
        assert_eq!(
            native.value(),
            &residual_affine_polynomial(&context, &expected_guard)
        );

        // Independent direct Symbolica full-point evaluation oracle.
        let ring = PolynomialRing::<IntegerRing, u16>::from_poly(&context.template.numerator);
        let direct = guard.raw.evaluate_with_coeff_map(
            |integer| context.template.numerator.constant(integer.clone()),
            &plan.core.full_images,
            &ring,
        );
        assert_eq!(native.value().raw, direct);
    }

    #[test]
    fn compact_affine_v2_supports_overlapping_images_and_exact_zero_guard() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "compact-affine-v2-overlapping-images",
            4,
        )
        .unwrap();
        // n0 and n1 deliberately have the same image 1+n2+n3.
        let constants = [
            Integer::one(),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
        ];
        let free_positions = [2usize, 3];
        let compact_linear_coefficients = [
            Integer::one(),
            Integer::one(),
            Integer::one(),
            Integer::one(),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        let view = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            4,
            &constants,
            &free_positions,
            &compact_linear_coefficients,
        );
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                view,
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        plan.replay(&context, view).unwrap();

        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let n3 = context.index(3).unwrap();
        let difference = context.sub(&n0, &n1).unwrap();
        let source_value = context
            .add(
                &context.mul(&difference, &difference).unwrap(),
                &context
                    .add(
                        &context.mul(&n0, &n1).unwrap(),
                        &context.mul(&n0, &n2).unwrap(),
                    )
                    .unwrap(),
            )
            .unwrap();
        let source = residual_affine_polynomial(&context, &source_value);
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let native = context
            .compose_guard_on_residual_affine_compact_composition_plan(&source, &plan, limits)
            .unwrap();
        let image = context
            .add(&context.add(&context.one(), &n2).unwrap(), &n3)
            .unwrap();
        let expected = context
            .add(
                &context.mul(&image, &image).unwrap(),
                &context.mul(&image, &n2).unwrap(),
            )
            .unwrap();
        assert_eq!(
            native.value(),
            &residual_affine_polynomial(&context, &expected)
        );

        let zero_guard = residual_affine_polynomial(&context, &difference);
        let mapped_zero = context
            .compose_guard_on_residual_affine_compact_composition_plan(&zero_guard, &plan, limits)
            .unwrap();
        assert!(mapped_zero.value().is_zero());
    }

    #[test]
    fn compact_affine_v2_reports_zero_mapped_denominator_without_provenance() {
        let context = residual_affine_test_context("compact-affine-v2-zero-denominator");
        let plan = residual_affine_compact_test_plan(&context);
        let equality = context
            .add(
                &context
                    .sub(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context
                    .sub(&context.index(2).unwrap(), &context.integer(2))
                    .unwrap(),
            )
            .unwrap();
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: context.template.numerator.one(),
                denominator: equality.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let mapped = context
            .compose_coefficient_on_residual_affine_compact_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            mapped,
            ResidualAffineCoefficientComposition::ZeroMappedDenominator { .. }
        ));
        assert_eq!(mapped.stats().denominator().output_terms(), 0);
        assert_eq!(mapped.stats().durable_denominator_terms(), 0);
        assert_eq!(mapped.stats().durable_guard_origin_retained_bytes(), 0);
    }

    #[test]
    fn compact_affine_v2_rejects_wrong_context_arity_and_map_shape() {
        let context = residual_affine_test_context("compact-affine-v2-shape-context");
        let foreign = residual_affine_test_context("compact-affine-v2-shape-foreign");
        let constants = [Integer::from(2), Integer::zero(), Integer::zero()];
        let free_positions = [1usize, 2];
        let linear = [
            Integer::one(),
            Integer::from(-1),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        let limits = ResidualAffineCompactCompositionPlanLimits::default();
        let compile = |fingerprint, arity, constants: &[Integer], free: &[usize], linear| {
            context.compile_residual_affine_compact_composition_plan(
                ResidualAffineCompactMapView::new(fingerprint, arity, constants, free, linear),
                limits,
            )
        };
        assert!(matches!(
            compile(
                foreign.fingerprint(),
                3,
                &constants,
                &free_positions,
                &linear
            ),
            Err(ResidualUnitAffineCompositionError::WrongContext)
        ));
        assert!(matches!(
            compile(
                context.fingerprint(),
                2,
                &constants,
                &free_positions,
                &linear
            ),
            Err(ResidualUnitAffineCompositionError::WrongArity {
                expected: 3,
                actual: 2
            })
        ));
        assert!(matches!(
            compile(
                context.fingerprint(),
                3,
                &constants[..2],
                &free_positions,
                &linear,
            ),
            Err(ResidualUnitAffineCompositionError::InvalidCompactGeometry { .. })
        ));
        assert!(matches!(
            compile(
                context.fingerprint(),
                3,
                &constants,
                &free_positions,
                &linear[..5],
            ),
            Err(ResidualUnitAffineCompositionError::InvalidCompactGeometry { .. })
        ));
        let unsorted = [2usize, 1];
        assert!(matches!(
            compile(context.fingerprint(), 3, &constants, &unsorted, &linear),
            Err(ResidualUnitAffineCompositionError::InvalidCompactGeometry { .. })
        ));
        let mut nonidentity = linear.clone();
        nonidentity[2] = Integer::from(2);
        assert!(matches!(
            compile(
                context.fingerprint(),
                3,
                &constants,
                &free_positions,
                &nonidentity,
            ),
            Err(ResidualUnitAffineCompositionError::InvalidCompactGeometry { .. })
        ));
    }

    #[test]
    fn compact_affine_v2_dense_replay_has_single_pass_work_and_gmp_census() {
        const ARITY: usize = 32;
        const FREE_COUNT: usize = 16;
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "compact-affine-v2-dense-replay-work",
            ARITY,
        )
        .unwrap();
        let constants = vec![Integer::zero(); ARITY];
        let free_positions = (ARITY - FREE_COUNT..ARITY).collect::<Vec<_>>();
        let huge = (Integer::one() << 512u32) + Integer::from(37);
        let mut linear = Vec::with_capacity(ARITY * FREE_COUNT);
        for row in 0..ARITY {
            for free_ordinal in 0..FREE_COUNT {
                linear.push(if row < ARITY - FREE_COUNT {
                    huge.clone()
                } else if free_ordinal == row - (ARITY - FREE_COUNT) {
                    Integer::one()
                } else {
                    Integer::zero()
                });
            }
        }
        let view = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            ARITY,
            &constants,
            &free_positions,
            &linear,
        );
        let plan = context
            .compile_residual_affine_compact_composition_plan(
                view,
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        plan.replay(&context, view).unwrap();

        let stats = plan.stats();
        let composition = stats.composition();
        let variable_count = context.base.variables().len() + ARITY;
        let free_slot_count = FREE_COUNT + 1;
        let expected_comparison_work = (ARITY + FREE_COUNT + free_slot_count)
            + ARITY
            + variable_count
            + composition.geometry_entries_retained()
            + composition.total_image_terms()
            + composition.total_image_exponent_entries();
        assert_eq!(
            stats.geometry_replay_comparison_work(),
            expected_comparison_work
        );
        assert_eq!(
            stats.geometry_replay_integer_bit_work(),
            stats.geometry_integer_bit_work() + composition.total_image_integer_bits()
        );
        let old_occurrence_rescan_exponent_work =
            (ARITY - FREE_COUNT) * FREE_COUNT * FREE_COUNT * variable_count;
        assert!(
            stats.geometry_replay_comparison_work() < old_occurrence_rescan_exponent_work,
            "deep replay must remain one-pass rather than rescan each dense image per coefficient"
        );
    }

    #[test]
    fn compact_affine_v2_gmp_plan_limits_accept_exact_and_reject_every_positive_one_below() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "compact-affine-v2-gmp-exact-limits",
            2,
        )
        .unwrap();
        let huge_constant = (Integer::one() << 4096u32) + Integer::from(3);
        let huge_linear = (Integer::one() << 3072u32) + Integer::from(5);
        let constants = [huge_constant, Integer::zero()];
        let free_positions = [1usize];
        let linear = [huge_linear, Integer::one()];
        let view = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            2,
            &constants,
            &free_positions,
            &linear,
        );
        let reference = context
            .compile_residual_affine_compact_composition_plan(
                view,
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        let stats = reference.stats();
        let composition = stats.composition();
        let exact = ResidualAffineCompactCompositionPlanLimits {
            composition: ResidualUnitAffineCompositionPlanLimits {
                max_variables: composition.variables(),
                max_full_images: composition.full_images(),
                max_geometry_entries_inspected: composition.geometry_entries_inspected(),
                max_geometry_entries_retained: composition.geometry_entries_retained(),
                max_support_entries_retained: composition.support_entries_retained(),
                max_total_image_terms: composition.total_image_terms(),
                max_total_image_exponent_entries: composition.total_image_exponent_entries(),
                max_image_integer_bits: composition.largest_image_integer_bits(),
                max_total_image_integer_bits: composition.total_image_integer_bits(),
            },
            max_context_fingerprint_bytes: stats.context_fingerprint_bytes(),
            max_geometry_integer_bit_work: stats.geometry_integer_bit_work(),
            max_geometry_replay_comparison_work: stats.geometry_replay_comparison_work(),
            max_geometry_replay_integer_bit_work: stats.geometry_replay_integer_bit_work(),
            max_geometry_replay_scratch_logical_bytes: stats
                .geometry_replay_scratch_logical_bytes(),
            max_retained_owned_logical_bytes: stats.retained_owned_logical_bytes(),
            max_compilation_owned_logical_peak_upper_bound: stats
                .compilation_owned_logical_peak_upper_bound(),
        };
        let exact_plan = context
            .compile_residual_affine_compact_composition_plan(view, exact)
            .unwrap();
        assert_eq!(exact_plan.stats(), stats);

        macro_rules! reject_composition_one_below {
            ($field:ident, $value:expr) => {{
                let value = $value;
                assert!(value > 0, "{} must be positive", stringify!($field));
                let mut below = exact;
                below.composition.$field = value - 1;
                assert!(matches!(
                    context.compile_residual_affine_compact_composition_plan(view, below),
                    Err(ResidualUnitAffineCompositionError::ResourceLimit { .. })
                ));
            }};
        }
        reject_composition_one_below!(max_variables, composition.variables());
        reject_composition_one_below!(max_full_images, composition.full_images());
        reject_composition_one_below!(
            max_geometry_entries_inspected,
            composition.geometry_entries_inspected()
        );
        reject_composition_one_below!(
            max_geometry_entries_retained,
            composition.geometry_entries_retained()
        );
        reject_composition_one_below!(
            max_support_entries_retained,
            composition.support_entries_retained()
        );
        reject_composition_one_below!(max_total_image_terms, composition.total_image_terms());
        reject_composition_one_below!(
            max_total_image_exponent_entries,
            composition.total_image_exponent_entries()
        );
        reject_composition_one_below!(
            max_image_integer_bits,
            composition.largest_image_integer_bits()
        );
        reject_composition_one_below!(
            max_total_image_integer_bits,
            composition.total_image_integer_bits()
        );

        macro_rules! reject_outer_one_below {
            ($field:ident, $value:expr) => {{
                let value = $value;
                assert!(value > 0, "{} must be positive", stringify!($field));
                let mut below = exact;
                below.$field = value - 1;
                assert!(matches!(
                    context.compile_residual_affine_compact_composition_plan(view, below),
                    Err(ResidualUnitAffineCompositionError::ResourceLimit { .. })
                ));
            }};
        }
        reject_outer_one_below!(
            max_context_fingerprint_bytes,
            stats.context_fingerprint_bytes()
        );
        reject_outer_one_below!(
            max_geometry_integer_bit_work,
            stats.geometry_integer_bit_work()
        );
        reject_outer_one_below!(
            max_geometry_replay_comparison_work,
            stats.geometry_replay_comparison_work()
        );
        reject_outer_one_below!(
            max_geometry_replay_integer_bit_work,
            stats.geometry_replay_integer_bit_work()
        );
        reject_outer_one_below!(
            max_geometry_replay_scratch_logical_bytes,
            stats.geometry_replay_scratch_logical_bytes()
        );
        reject_outer_one_below!(
            max_retained_owned_logical_bytes,
            stats.retained_owned_logical_bytes()
        );
        reject_outer_one_below!(
            max_compilation_owned_logical_peak_upper_bound,
            stats.compilation_owned_logical_peak_upper_bound()
        );
    }

    #[test]
    fn compact_affine_v2_plan_is_lifetime_safe_exactly_replayable_tamper_evident_and_redacted() {
        let scope_sentinel = "compact-affine-v2-private-context-sentinel";
        let context = residual_affine_test_context(scope_sentinel);
        let huge = (Integer::one() << 300u32) + Integer::from(41);
        let huge_rendered = huge.to_string();
        let plan = {
            let constants = [huge.clone(), Integer::zero(), Integer::zero()];
            let free_positions = [1usize, 2];
            let linear = [
                Integer::one(),
                Integer::from(-1),
                Integer::one(),
                Integer::zero(),
                Integer::zero(),
                Integer::one(),
            ];
            context
                .compile_residual_affine_compact_composition_plan(
                    ResidualAffineCompactMapView::new(
                        context.fingerprint(),
                        3,
                        &constants,
                        &free_positions,
                        &linear,
                    ),
                    ResidualAffineCompactCompositionPlanLimits::default(),
                )
                .unwrap()
        };

        let constants = [huge.clone(), Integer::zero(), Integer::zero()];
        let free_positions = [1usize, 2];
        let linear = [
            Integer::one(),
            Integer::from(-1),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        let view = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            3,
            &constants,
            &free_positions,
            &linear,
        );
        plan.replay(&context, view).unwrap();
        assert_eq!(plan.ambient_arity(), 3);
        assert_eq!(plan.free_positions(), free_positions);
        assert_eq!(
            plan.manifest().schema(),
            RESIDUAL_AFFINE_COMPACT_COMPOSITION_V2_SCHEMA
        );
        assert_eq!(plan.manifest().ambient_arity(), 3);
        assert_eq!(plan.manifest().free_count(), 2);
        assert_eq!(plan.manifest().limits(), plan.limits());
        assert_eq!(plan.manifest().stats(), plan.stats());
        assert_eq!(
            plan.manifest().context_fingerprint_bytes(),
            context.fingerprint().len()
        );
        assert_ne!(plan.manifest().context_checksum(), 0);
        assert_ne!(plan.manifest().geometry_checksum(), 0);
        assert!(
            plan.owned_retained_byte_bound().unwrap()
                >= plan.stats().retained_owned_logical_bytes()
        );

        let mut changed_constants = constants.clone();
        changed_constants[0] += Integer::one();
        let changed_view = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            3,
            &changed_constants,
            &free_positions,
            &linear,
        );
        assert!(matches!(
            plan.replay(&context, changed_view),
            Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)
        ));

        let mut tampered_core = plan.clone();
        let base_count = context.base.variables().len();
        Arc::make_mut(&mut tampered_core.core).full_images[base_count].coefficients[0] =
            Integer::from(17);
        assert!(matches!(
            tampered_core.replay(&context, view),
            Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)
        ));
        let mut duplicate_monomial = plan.clone();
        let variable_count = context.variables.len();
        let image = &mut Arc::make_mut(&mut duplicate_monomial.core).full_images[base_count];
        assert!(image.nterms() >= 2);
        let first = image.exponents[..variable_count].to_vec();
        image.exponents[variable_count..2 * variable_count].copy_from_slice(&first);
        assert!(matches!(
            duplicate_monomial.replay(&context, view),
            Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)
        ));
        let mut tampered_manifest = plan.clone();
        tampered_manifest.manifest.geometry_checksum ^= 1;
        assert!(matches!(
            tampered_manifest.replay(&context, view),
            Err(ResidualUnitAffineCompositionError::SchemaMismatch)
        ));
        // Even a coherently forged diagnostic checksum cannot replace deep
        // componentwise replay against the canonical Symbolica images.
        let mut coherently_forged_checksum = plan.clone();
        let changed_checksum = residual_affine_compact_geometry_checksum(changed_view);
        coherently_forged_checksum.geometry_checksum = changed_checksum;
        coherently_forged_checksum.manifest.geometry_checksum = changed_checksum;
        assert!(matches!(
            coherently_forged_checksum.replay(&context, changed_view),
            Err(ResidualUnitAffineCompositionError::CompactGeometryReplayMismatch)
        ));

        let rendered = format!("{view:?} {plan:?} {:?}", plan.manifest());
        assert!(!rendered.contains(scope_sentinel));
        assert!(!rendered.contains(context.fingerprint()));
        assert!(!rendered.contains(&huge_rendered));

        let forbidden = ["un", "safe"].concat();
        assert!(!include_str!("parametric_coefficient.rs").contains(&forbidden));
    }

    #[test]
    fn compact_affine_v2_contains_boundary_panics_and_replays_composes_concurrently() {
        let context = residual_affine_test_context("compact-affine-v2-panic-concurrent");
        let constants = [Integer::from(2), Integer::zero(), Integer::zero()];
        let free_positions = [1usize, 2];
        let linear = [
            Integer::one(),
            Integer::from(-1),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        let view = ResidualAffineCompactMapView::new(
            context.fingerprint(),
            3,
            &constants,
            &free_positions,
            &linear,
        );

        inject_residual_affine_compact_boundary_panic_for_test();
        assert!(matches!(
            context.compile_residual_affine_compact_composition_plan(
                view,
                ResidualAffineCompactCompositionPlanLimits::default(),
            ),
            Err(ResidualUnitAffineCompositionError::SymbolicaPanic {
                stage: "compact affine composition plan compilation"
            })
        ));

        let plan = context
            .compile_residual_affine_compact_composition_plan(
                view,
                ResidualAffineCompactCompositionPlanLimits::default(),
            )
            .unwrap();
        inject_residual_affine_compact_boundary_panic_for_test();
        assert!(matches!(
            plan.replay(&context, view),
            Err(ResidualUnitAffineCompositionError::SymbolicaPanic {
                stage: "compact affine composition plan replay"
            })
        ));

        let source = context
            .add(
                &context
                    .mul(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.index(2).unwrap(),
            )
            .unwrap();
        let guard = residual_affine_polynomial(&context, &source);
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let expected = context
            .compose_guard_on_residual_affine_compact_composition_plan(&guard, &plan, limits)
            .unwrap();
        std::thread::scope(|scope| {
            for _ in 0..4 {
                let plan = plan.clone();
                let expected = expected.clone();
                let guard = guard.clone();
                let context = &context;
                scope.spawn(move || {
                    plan.replay(&context, view).unwrap();
                    let observed = context
                        .compose_guard_on_residual_affine_compact_composition_plan(
                            &guard, &plan, limits,
                        )
                        .unwrap();
                    assert_eq!(observed.value(), expected.value());
                    assert_eq!(observed.stats(), expected.stats());
                });
            }
        });
    }

    #[test]
    fn fresh_integer_system_plan_skips_only_replay_and_preserves_arc_identity() {
        let empty_context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "fresh-integer-system-plan-identity",
            1,
        )
        .unwrap();
        let baseline = ResidualAffineIntegerSystemCertificate::compile(
            1,
            &[],
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap();
        let mut replay_limited = ResidualAffineIntegerSystemLimits::default();
        replay_limited.max_verification_operations = baseline.stats().verification_operations();
        let empty_fresh = match ResidualAffineIntegerSystemCertificate::compile_fresh(
            1,
            &[],
            replay_limited,
        )
        .unwrap()
        {
            crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(fresh) => fresh,
            crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(_) => {
                panic!("empty integer system unexpectedly unsupported")
            }
        };
        let empty_certificate = Arc::new(
            ResidualAffineIntegerSystemCertificate::compile(1, &[], replay_limited).unwrap(),
        );
        match empty_context
            .compile_residual_affine_composition_plan_from_integer_system(
                Arc::clone(&empty_certificate),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap_err()
        {
            ResidualUnitAffineCompositionError::IntegerSystem(
                ResidualAffineIntegerSystemError::ResourceLimit {
                    resource: "verification operations",
                    requested,
                    limit,
                },
            ) => {
                assert_eq!(limit, baseline.stats().verification_operations());
                assert_eq!(requested, limit + 1);
            }
            other => panic!("unexpected legacy replay failure: {other:?}"),
        }
        let (fresh_empty_certificate, empty_authorization) = empty_fresh
            .into_certificate_and_plan_authorization()
            .unwrap();
        assert!(empty_authorization.authenticates_certificate_allocation(&fresh_empty_certificate));
        assert!(!empty_authorization.authenticates_certificate_allocation(&empty_certificate));
        let empty_plan = empty_context
            .compile_residual_affine_composition_plan_from_fresh_integer_system(
                empty_authorization,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        assert!(Arc::ptr_eq(
            empty_plan.certificate(),
            &fresh_empty_certificate
        ));
        assert_eq!(empty_plan.stats().variables(), 2);

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "fresh-integer-system-plan-shared-inner",
            3,
        )
        .unwrap();
        let rows = vec![residual_affine_integer_system_row(
            vec![3, -1, -1, 0].into_iter().map(Integer::from).collect(),
            0,
        )];
        let fresh = match ResidualAffineIntegerSystemCertificate::compile_fresh(
            3,
            &rows,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap()
        {
            crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(fresh) => fresh,
            crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(_) => {
                panic!("unit-pivot integer system unexpectedly unsupported")
            }
        };
        let legacy_certificate = Arc::new(
            ResidualAffineIntegerSystemCertificate::compile(
                3,
                &rows,
                ResidualAffineIntegerSystemLimits::default(),
            )
            .unwrap(),
        );
        let legacy = context
            .compile_residual_affine_composition_plan_from_integer_system(
                Arc::clone(&legacy_certificate),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let (fresh_certificate, authorization) =
            fresh.into_certificate_and_plan_authorization().unwrap();
        assert!(authorization.authenticates_certificate_allocation(&fresh_certificate));
        assert!(!authorization.authenticates_certificate_allocation(&legacy_certificate));
        let sealed = context
            .compile_residual_affine_composition_plan_from_fresh_integer_system(
                authorization,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        assert!(Arc::ptr_eq(legacy.certificate(), &legacy_certificate));
        assert!(Arc::ptr_eq(sealed.certificate(), &fresh_certificate));
        assert_eq!(sealed.stats(), legacy.stats());
        assert_eq!(sealed.core.free_positions, legacy.core.free_positions);
        assert_eq!(sealed.core.nonfree_positions, legacy.core.nonfree_positions);
        assert_eq!(sealed.core.linear_support, legacy.core.linear_support);
        assert_eq!(sealed.core.full_images, legacy.core.full_images);

        let tampered_fresh = match ResidualAffineIntegerSystemCertificate::compile_fresh(
            3,
            &rows,
            ResidualAffineIntegerSystemLimits::default(),
        )
        .unwrap()
        {
            crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshCompilationAttempt::Complete(fresh) => fresh,
            crate::residual_affine_integer_system::ResidualAffineIntegerSystemFreshCompilationAttempt::Unsupported(_) => {
                panic!("unit-pivot integer system unexpectedly unsupported")
            }
        };
        let (_, mut tampered_authorization) = tampered_fresh
            .into_certificate_and_plan_authorization()
            .unwrap();
        tampered_authorization.tamper_payload_units_for_test();
        assert!(matches!(
            context.compile_residual_affine_composition_plan_from_fresh_integer_system(
                tampered_authorization,
                ResidualUnitAffineCompositionPlanLimits::default(),
            ),
            Err(ResidualUnitAffineCompositionError::IntegerSystem(
                ResidualAffineIntegerSystemError::ReplayMismatch
            ))
        ));
    }

    #[test]
    fn integer_system_affine_composition_handles_nonleading_overlapping_images() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d", "m2"]),
            "integer-system-affine-compose",
            4,
        )
        .unwrap();
        // n_0 = 5 + 2*n_1 + 3*n_3
        // n_2 = -7 - 2*n_1 + 4*n_3.  The coefficient 2 keeps n_1
        // ineligible as the second original-coordinate unit pivot.
        let certificate = residual_affine_integer_system_certificate(
            4,
            vec![
                residual_affine_integer_system_row(
                    vec![5, -1, 2, 0, 3]
                        .into_iter()
                        .map(Integer::from)
                        .collect(),
                    0,
                ),
                residual_affine_integer_system_row(
                    vec![7, 0, 2, 1, -4]
                        .into_iter()
                        .map(Integer::from)
                        .collect(),
                    1,
                ),
            ],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate.clone(),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        assert!(Arc::ptr_eq(plan.certificate(), &certificate));
        assert_eq!(plan.core.free_positions.as_slice(), &[1, 3]);
        assert_eq!(plan.core.nonfree_positions.as_slice(), &[0, 2]);
        assert_eq!(plan.stats().geometry_entries_inspected(), 20);
        assert_eq!(plan.stats().geometry_entries_retained(), 12);
        assert_eq!(plan.stats().support_entries_retained(), 12);

        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let n3 = context.index(3).unwrap();
        let source = residual_affine_polynomial(&context, &context.mul(&n0, &n2).unwrap());
        let mapped = context
            .compose_polynomial_on_residual_affine_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let n0_image = context
            .add(
                &context
                    .add(
                        &context.integer(5),
                        &context.mul(&context.integer(2), &n1).unwrap(),
                    )
                    .unwrap(),
                &context.mul(&context.integer(3), &n3).unwrap(),
            )
            .unwrap();
        let n2_image = context
            .add(
                &context
                    .sub(
                        &context.integer(-7),
                        &context.mul(&context.integer(2), &n1).unwrap(),
                    )
                    .unwrap(),
                &context.mul(&context.integer(4), &n3).unwrap(),
            )
            .unwrap();
        let expected =
            residual_affine_polynomial(&context, &context.mul(&n0_image, &n2_image).unwrap());
        assert_eq!(mapped.value(), &expected);
        let (mapped_value, mapped_stats) = mapped.into_parts();
        assert_eq!(mapped_value, expected);
        assert!(mapped_stats.expanded_contribution_bound() >= mapped_stats.output_terms());
        let base_count = context.base.variables().len();
        assert!(
            mapped_value
                .raw
                .exponents_iter()
                .all(|exponents| { exponents[base_count] == 0 && exponents[base_count + 2] == 0 })
        );
    }

    #[test]
    fn symbolica_polynomial_evaluator_matches_exact_multinomial_fixture() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-polynomial-evaluator-exact-multinomial",
            3,
        )
        .unwrap();
        // 2 - n0 + 3*n1 - 5*n2 = 0, so n0 = 2 + 3*n1 - 5*n2.
        let certificate = residual_affine_integer_system_certificate(
            3,
            vec![residual_affine_integer_system_row(
                [2, -1, 3, -5].into_iter().map(Integer::from).collect(),
                0,
            )],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = context.index(0).unwrap();
        let n0_squared = context.mul(&n0, &n0).unwrap();
        let source =
            residual_affine_polynomial(&context, &context.mul(&n0_squared, &n0_squared).unwrap());
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let preflight = context
            .preflight_residual_affine_polynomial_core(&source, &plan.core, limits)
            .unwrap();
        assert_eq!(
            preflight.backend,
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator
        );
        let native = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &plan, limits)
            .unwrap();
        assert_eq!(native.stats().expanded_contribution_bound(), 15);
        assert_eq!(native.stats().output_terms(), 15);

        let coefficient_at = |target: &[u16]| {
            native
                .value()
                .raw
                .exponents_iter()
                .zip(&native.value().raw.coefficients)
                .find_map(|(exponents, coefficient)| (exponents == target).then_some(coefficient))
                .cloned()
        };
        // Complete independent coefficient table for
        // (2 + 3*n1 - 5*n2)^4. This restores full-output differential
        // coverage without retaining a second polynomial compositor.
        for (exponents, coefficient) in [
            ([0, 0, 0], 16),
            ([0, 1, 0], 96),
            ([0, 0, 1], -160),
            ([0, 2, 0], 216),
            ([0, 1, 1], -720),
            ([0, 0, 2], 600),
            ([0, 3, 0], 216),
            ([0, 2, 1], -1_080),
            ([0, 1, 2], 1_800),
            ([0, 0, 3], -1_000),
            ([0, 4, 0], 81),
            ([0, 3, 1], -540),
            ([0, 2, 2], 1_350),
            ([0, 1, 3], -1_500),
            ([0, 0, 4], 625),
        ] {
            assert_eq!(
                coefficient_at(&exponents),
                Some(Integer::from(coefficient)),
                "wrong quartic coefficient at {exponents:?}"
            );
        }

        macro_rules! reject_one_below {
            ($field:ident, $exact:expr) => {{
                let exact = $exact;
                assert!(exact > 0, "{} must be positive", stringify!($field));
                let strict = ResidualUnitAffinePolynomialCompositionLimits {
                    $field: exact - 1,
                    ..limits
                };
                assert!(
                    matches!(
                        context.preflight_polynomial_on_residual_affine_composition_plan(
                            &source, &plan, strict,
                        ),
                        Err(ResidualUnitAffineCompositionError::ResourceLimit { .. })
                    ),
                    "{} accepted one below its exact Symbolica preflight census",
                    stringify!($field),
                );
            }};
        }
        let stats = native.stats();
        reject_one_below!(max_power_calls, stats.power_calls());
        reject_one_below!(
            max_native_power_heap_pairs,
            stats.native_power_heap_pair_bound()
        );
        reject_one_below!(
            max_multiplication_term_pairs,
            stats.multiplication_term_pair_bound()
        );
        reject_one_below!(
            max_expanded_contributions,
            stats.expanded_contribution_bound()
        );
        reject_one_below!(max_output_terms, stats.expanded_contribution_bound());
        reject_one_below!(
            max_output_exponent_entries,
            stats.output_exponent_entry_bound()
        );
        reject_one_below!(max_addition_term_visits, stats.addition_term_visit_bound());
        reject_one_below!(
            max_integer_coefficient_bits,
            stats.largest_integer_coefficient_bit_bound()
        );
        reject_one_below!(max_integer_bit_work, stats.integer_bit_work_bound());
    }

    #[test]
    fn symbolica_backend_expression_composition_produces_528_terms_beyond_u32_key() {
        const ARITY: usize = 33;
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-expression-beyond-native-u32-kronecker-key",
            ARITY,
        )
        .unwrap();
        // n0-n1-...-n32=0.  Squaring n0 after substitution yields 32
        // squares and C(32,2)=496 pair monomials.
        let mut components = Vec::with_capacity(ARITY + 1);
        components.push(Integer::zero());
        components.push(Integer::one());
        components.extend((1..ARITY).map(|_| Integer::from(-1)));
        let certificate = residual_affine_integer_system_certificate(
            ARITY,
            vec![residual_affine_integer_system_row(components, 0)],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = context.index(0).unwrap();
        let source = residual_affine_polynomial(&context, &context.mul(&n0, &n0).unwrap());
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let preflight = context
            .preflight_residual_affine_polynomial_core(&source, &plan.core, limits)
            .unwrap();
        assert_eq!(
            preflight.backend,
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion
        );
        let expression_composed = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &plan, limits)
            .unwrap();

        assert_eq!(
            expression_composed.stats().expanded_contribution_bound(),
            528
        );
        assert_eq!(expression_composed.stats().output_terms(), 528);
        assert_eq!(expression_composed.value().raw.nterms(), 528);

        let strict = ResidualUnitAffinePolynomialCompositionLimits {
            max_kronecker_exponent_bits: preflight
                .stats
                .largest_kronecker_exponent_bits()
                .checked_sub(1)
                .unwrap(),
            ..limits
        };
        assert!(matches!(
            context
                .preflight_polynomial_on_residual_affine_composition_plan(&source, &plan, strict,),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "Kronecker exponent bits",
                ..
            })
        ));

        let exact_integer_work = preflight.stats.integer_bit_work_bound();
        assert!(exact_integer_work > 0);
        let strict_expression_work = ResidualUnitAffinePolynomialCompositionLimits {
            max_integer_bit_work: exact_integer_work - 1,
            ..limits
        };
        assert!(matches!(
            context.preflight_polynomial_on_residual_affine_composition_plan(
                &source,
                &plan,
                strict_expression_work,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer bit work",
                requested,
                limit,
            }) if requested == exact_integer_work && limit + 1 == exact_integer_work
        ));

        let rows: Vec<_> = expression_composed.value().raw.exponents_iter().collect();
        assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
        let mut squares = 0usize;
        let mut pairs = 0usize;
        for (exponents, coefficient) in rows
            .into_iter()
            .zip(&expression_composed.value().raw.coefficients)
        {
            assert_eq!(exponents[0], 0);
            assert_eq!(
                exponents
                    .iter()
                    .map(|&value| usize::from(value))
                    .sum::<usize>(),
                2
            );
            let nonzero: Vec<_> = exponents[1..]
                .iter()
                .copied()
                .filter(|&value| value != 0)
                .collect();
            match nonzero.as_slice() {
                [2] => {
                    squares += 1;
                    assert_eq!(coefficient, &Integer::one());
                }
                [1, 1] => {
                    pairs += 1;
                    assert_eq!(coefficient, &Integer::from(2));
                }
                other => panic!("unexpected degree-two exponent support: {other:?}"),
            }
        }
        assert_eq!(squares, 32);
        assert_eq!(pairs, 496);
    }

    #[test]
    fn symbolica_expression_structural_visits_accept_exact_and_reject_one_below() {
        const ARITY: usize = 42;
        const PIVOT_COUNT: usize = 21;
        const EXPECTED_OUTPUT_TERMS: usize = 231;

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-expression-structural-visits",
            ARITY,
        )
        .unwrap();
        // n0,...,n19 vanish, while n20 is the sum of the 21 free
        // coordinates n21,...,n41. The source mentions n20 only, so its
        // support is discovered only after probing all R=21 nonfree
        // positions. The executor then sends just this one active
        // replacement to Symbolica. Squaring its 21-term image also crosses
        // Symbolica's u32 polynomial-evaluator stride.
        let mut rows = Vec::with_capacity(PIVOT_COUNT);
        for pivot in 0..PIVOT_COUNT {
            let mut components = vec![Integer::zero(); ARITY + 1];
            components[pivot + 1] = Integer::one();
            if pivot + 1 == PIVOT_COUNT {
                for free in PIVOT_COUNT..ARITY {
                    components[free + 1] = Integer::from(-1);
                }
            }
            rows.push(residual_affine_integer_system_row(components, pivot));
        }
        let certificate = residual_affine_integer_system_certificate(ARITY, rows);
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        assert_eq!(
            plan.core.free_positions.as_slice(),
            &(PIVOT_COUNT..ARITY).collect::<Vec<_>>()
        );
        assert_eq!(
            plan.core.nonfree_positions.as_slice(),
            &(0..PIVOT_COUNT).collect::<Vec<_>>()
        );

        let pivot = context.index(PIVOT_COUNT - 1).unwrap();
        let source = residual_affine_polynomial(&context, &context.mul(&pivot, &pivot).unwrap());
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let preflight = context
            .preflight_residual_affine_polynomial_core(&source, &plan.core, limits)
            .unwrap();
        assert_eq!(
            preflight.backend,
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion
        );
        assert_eq!(
            preflight.stats.expanded_contribution_bound(),
            EXPECTED_OUTPUT_TERMS
        );
        // Independently spell the selected-backend census for this fixture:
        // F + Q + K*(Nsrc + image_build + Nsub + 2P + M*V + 4C + 6E).
        let source_nodes = 1 + 2 + 3 * ARITY;
        let support_filter_probes = 3 * PIVOT_COUNT;
        let replacement_attempts = source_nodes;
        let image_build = PIVOT_COUNT * ARITY + ARITY + (1 + 3 * PIVOT_COUNT);
        let substituted_nodes = source_nodes + 3 * PIVOT_COUNT;
        let power_heap_pairs = PIVOT_COUNT * EXPECTED_OUTPUT_TERMS;
        let output_exponent_entries = EXPECTED_OUTPUT_TERMS * ARITY;
        assert!(power_heap_pairs > EXPECTED_OUTPUT_TERMS);
        let sort_factor = 4 * (residual_affine_ceil_log2(power_heap_pairs) + 1);
        let expected_structural_visits = support_filter_probes
            + replacement_attempts
            + sort_factor
                * (source_nodes
                    + image_build
                    + substituted_nodes
                    + 2 * power_heap_pairs
                    + EXPECTED_OUTPUT_TERMS * ARITY
                    + 4 * EXPECTED_OUTPUT_TERMS
                    + 6 * output_exponent_entries);
        assert_eq!(expected_structural_visits, 4_471_736);
        assert_eq!(
            preflight.stats.addition_term_visit_bound(),
            expected_structural_visits
        );

        let exact = ResidualUnitAffinePolynomialCompositionLimits {
            max_addition_term_visits: expected_structural_visits,
            ..limits
        };
        context
            .preflight_polynomial_on_residual_affine_composition_plan(&source, &plan, exact)
            .expect("the exact Symbolica expression structural-work boundary must pass");
        let composed = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &plan, exact)
            .unwrap();
        assert_eq!(composed.value().raw.nterms(), EXPECTED_OUTPUT_TERMS);
        let mut squares = 0usize;
        let mut pairs = 0usize;
        for (exponents, coefficient) in composed
            .value()
            .raw
            .exponents_iter()
            .zip(&composed.value().raw.coefficients)
        {
            assert!(exponents[..PIVOT_COUNT].iter().all(|&value| value == 0));
            let support: Vec<_> = exponents[PIVOT_COUNT..]
                .iter()
                .copied()
                .filter(|&value| value != 0)
                .collect();
            match support.as_slice() {
                [2] => {
                    squares += 1;
                    assert_eq!(coefficient, &Integer::one());
                }
                [1, 1] => {
                    pairs += 1;
                    assert_eq!(coefficient, &Integer::from(2));
                }
                other => panic!("unexpected expression-backend support: {other:?}"),
            }
        }
        assert_eq!(squares, PIVOT_COUNT);
        assert_eq!(pairs, PIVOT_COUNT * (PIVOT_COUNT - 1) / 2);

        let one_below = ResidualUnitAffinePolynomialCompositionLimits {
            max_addition_term_visits: expected_structural_visits - 1,
            ..limits
        };
        assert!(matches!(
            context.preflight_polynomial_on_residual_affine_composition_plan(
                &source, &plan, one_below,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "Symbolica backend structural term visits",
                requested,
                limit,
            }) if requested == expected_structural_visits
                && limit + 1 == expected_structural_visits
        ));
    }

    #[test]
    fn symbolica_expression_gmp_cancellation_has_exact_integer_work_boundary() {
        const ARITY: usize = 23;
        const FREE_START: usize = 2;

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-expression-gmp-cancellation",
            ARITY,
        )
        .unwrap();
        // Both pivots map to the same sum of 21 free variables. Each square
        // independently crosses the u32 Kronecker stride, while their
        // difference cancels only after Symbolica's simultaneous expansion.
        let rows = (0..FREE_START)
            .map(|pivot| {
                let mut components = vec![Integer::zero(); ARITY + 1];
                components[pivot + 1] = Integer::one();
                for free in FREE_START..ARITY {
                    components[free + 1] = Integer::from(-1);
                }
                residual_affine_integer_system_row(components, pivot)
            })
            .collect();
        let certificate = residual_affine_integer_system_certificate(ARITY, rows);
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        assert_eq!(
            plan.core.free_positions.as_slice(),
            &(FREE_START..ARITY).collect::<Vec<_>>()
        );

        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let difference = context
            .sub(
                &context.mul(&n0, &n0).unwrap(),
                &context.mul(&n1, &n1).unwrap(),
            )
            .unwrap();
        let huge = (Integer::one() << 256u32) + Integer::from(17);
        let huge_coefficient =
            context.wrap_unchecked(context.template.numerator.constant(huge).into());
        let source = residual_affine_polynomial(
            &context,
            &context.mul(&huge_coefficient, &difference).unwrap(),
        );
        let wide = ResidualUnitAffinePolynomialCompositionLimits {
            max_integer_bit_work: usize::MAX,
            ..ResidualUnitAffinePolynomialCompositionLimits::default()
        };
        let preflight = context
            .preflight_residual_affine_polynomial_core(&source, &plan.core, wide)
            .unwrap();
        assert_eq!(
            preflight.backend,
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion
        );
        assert!(preflight.stats.expanded_contribution_bound() > 0);
        assert!(preflight.stats.largest_integer_coefficient_bit_bound() > 256);
        let packed_integer_width = preflight
            .stats
            .largest_integer_coefficient_bit_bound()
            .max(u16::BITS as usize);
        assert!(
            preflight.stats.native_integer_bit_work_bound()
                >= preflight
                    .stats
                    .addition_term_visit_bound()
                    .checked_mul(packed_integer_width)
                    .unwrap(),
            "the expression backend must admit packed integer payload work for every structural visit"
        );

        let exact_integer_work = preflight.stats.integer_bit_work_bound();
        assert!(exact_integer_work > 0);
        let exact = ResidualUnitAffinePolynomialCompositionLimits {
            max_integer_bit_work: exact_integer_work,
            ..wide
        };
        let composed = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &plan, exact)
            .expect("the exact expression integer-work boundary must pass");
        assert!(composed.value().is_zero());
        assert_eq!(composed.stats().output_terms(), 0);

        let one_below = ResidualUnitAffinePolynomialCompositionLimits {
            max_integer_bit_work: exact_integer_work - 1,
            ..wide
        };
        assert!(matches!(
            context.preflight_polynomial_on_residual_affine_composition_plan(
                &source, &plan, one_below,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer bit work",
                requested,
                limit,
            }) if requested == exact_integer_work && limit + 1 == exact_integer_work
        ));
    }

    #[test]
    fn symbolica_expression_ignores_inactive_wide_replacement_rhs() {
        const ARITY: usize = 23;
        const FREE_START: usize = 2;

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-expression-inactive-wide-rhs",
            ARITY,
        )
        .unwrap();
        let compile = |inactive_coefficient: Integer| {
            let mut active = vec![Integer::zero(); ARITY + 1];
            active[1] = Integer::one();
            for free in FREE_START..ARITY {
                active[free + 1] = Integer::from(-1);
            }
            let mut inactive = vec![Integer::zero(); ARITY + 1];
            inactive[2] = Integer::one();
            inactive[FREE_START + 1] = -inactive_coefficient;
            context
                .compile_residual_affine_composition_plan_from_integer_system(
                    residual_affine_integer_system_certificate(
                        ARITY,
                        vec![
                            residual_affine_integer_system_row(active, 0),
                            residual_affine_integer_system_row(inactive, 1),
                        ],
                    ),
                    ResidualUnitAffineCompositionPlanLimits::default(),
                )
                .unwrap()
        };
        let first = compile((Integer::one() << 256u32) + Integer::from(17));
        let second = compile(Integer::one());
        let n0 = context.index(0).unwrap();
        let source = residual_affine_polynomial(&context, &context.mul(&n0, &n0).unwrap());
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let first_preflight = context
            .preflight_residual_affine_polynomial_core(&source, &first.core, limits)
            .unwrap();
        let second_preflight = context
            .preflight_residual_affine_polynomial_core(&source, &second.core, limits)
            .unwrap();
        assert_eq!(
            first_preflight.backend,
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion
        );
        assert_eq!(first_preflight.backend, second_preflight.backend);
        assert_eq!(first_preflight.stats, second_preflight.stats);

        let first_output = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &first, limits)
            .unwrap();
        let second_output = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &second, limits)
            .unwrap();
        assert_eq!(first_output.value(), second_output.value());
        assert_eq!(first_output.stats(), second_output.stats());
    }

    #[test]
    fn symbolica_expression_zero_killed_intermediate_keeps_power_and_product_work() {
        const ARITY: usize = 23;
        const FREE_START: usize = 2;
        const POWER_TERMS: usize = 231;

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-expression-zero-killed-intermediate",
            ARITY,
        )
        .unwrap();
        let mut wide = vec![Integer::zero(); ARITY + 1];
        wide[1] = Integer::one();
        for free in FREE_START..ARITY {
            wide[free + 1] = Integer::from(-1);
        }
        let mut zero = vec![Integer::zero(); ARITY + 1];
        zero[2] = Integer::one();
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                residual_affine_integer_system_certificate(
                    ARITY,
                    vec![
                        residual_affine_integer_system_row(wide, 0),
                        residual_affine_integer_system_row(zero, 1),
                    ],
                ),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let source = residual_affine_polynomial(
            &context,
            &context.mul(&context.mul(&n0, &n0).unwrap(), &n1).unwrap(),
        );
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let preflight = context
            .preflight_residual_affine_polynomial_core(&source, &plan.core, limits)
            .unwrap();
        assert_eq!(
            preflight.backend,
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion
        );
        assert_eq!(preflight.stats.expanded_contribution_bound(), 0);
        assert_eq!(
            preflight.stats.native_power_heap_pair_bound(),
            (ARITY - FREE_START) * POWER_TERMS
        );
        assert_eq!(
            preflight.stats.multiplication_term_pair_bound(),
            POWER_TERMS
        );

        let source_nodes = 1 + 2 + 3 * ARITY;
        let support_probes = 3 * FREE_START;
        let replacement_attempts = FREE_START * source_nodes;
        let image_terms = ARITY - FREE_START;
        let image_build = image_terms * ARITY + FREE_START * ARITY + (FREE_START + 3 * image_terms);
        let substituted_nodes = source_nodes + 3 * image_terms;
        let power_pairs = image_terms * POWER_TERMS;
        let product_factor_visits = POWER_TERMS * ARITY;
        let sort_factor = 4 * (residual_affine_ceil_log2(power_pairs) + 1);
        let expected_structural_visits = support_probes
            + replacement_attempts
            + sort_factor
                * (source_nodes
                    + image_build
                    + substituted_nodes
                    + 2 * power_pairs
                    + product_factor_visits);
        assert_eq!(expected_structural_visits, 885_846);
        assert_eq!(
            preflight.stats.addition_term_visit_bound(),
            expected_structural_visits
        );

        let exact = ResidualUnitAffinePolynomialCompositionLimits {
            max_addition_term_visits: expected_structural_visits,
            ..limits
        };
        let mapped = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &plan, exact)
            .unwrap();
        assert!(mapped.value().is_zero());
        assert_eq!(mapped.stats().output_terms(), 0);
        let one_below = ResidualUnitAffinePolynomialCompositionLimits {
            max_addition_term_visits: expected_structural_visits - 1,
            ..limits
        };
        assert!(matches!(
            context.preflight_polynomial_on_residual_affine_composition_plan(
                &source, &plan, one_below,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "Symbolica backend structural term visits",
                requested,
                limit,
            }) if requested == expected_structural_visits
                && limit + 1 == expected_structural_visits
        ));
    }

    #[test]
    fn symbolica_backend_respects_exact_u32_mixed_radix_boundary() {
        let classify = |arity: usize, pivot: usize| {
            let label = format!("symbolica-expression-radix-boundary-{arity}-{pivot}");
            let context = ParametricCoefficientContext::try_new(
                &CoefficientContext::new(Vec::<String>::new()),
                &label,
                arity,
            )
            .unwrap();
            // Only the requested coordinate is a unit pivot. This makes the
            // target coordinate order, rather than the solver's pivot choice,
            // the sole difference between the paired fixtures.
            let mut components = vec![
                if pivot == 0 {
                    Integer::from(-2)
                } else {
                    Integer::from(2)
                };
                arity + 1
            ];
            components[0] = Integer::zero();
            components[pivot + 1] = if pivot == 0 {
                Integer::one()
            } else {
                Integer::from(-1)
            };
            let certificate = residual_affine_integer_system_certificate(
                arity,
                vec![residual_affine_integer_system_row(components, 0)],
            );
            let plan = context
                .compile_residual_affine_composition_plan_from_integer_system(
                    certificate,
                    ResidualUnitAffineCompositionPlanLimits::default(),
                )
                .unwrap();
            let pivot_variable = context.index(pivot).unwrap();
            let source = residual_affine_polynomial(
                &context,
                &context.mul(&pivot_variable, &pivot_variable).unwrap(),
            );
            let preflight = context
                .preflight_residual_affine_polynomial_core(
                    &source,
                    &plan.core,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap();
            let composed = context
                .compose_polynomial_on_residual_affine_composition_plan(
                    &source,
                    &plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap();
            assert_eq!(composed.value().raw.nterms(), arity * (arity - 1) / 2);
            if arity == 22 {
                let mut squares = 0usize;
                let mut pairs = 0usize;
                for (exponents, coefficient) in composed
                    .value()
                    .raw
                    .exponents_iter()
                    .zip(&composed.value().raw.coefficients)
                {
                    let support: Vec<_> = exponents
                        .iter()
                        .copied()
                        .filter(|&value| value != 0)
                        .collect();
                    match support.as_slice() {
                        [2] => {
                            squares += 1;
                            assert_eq!(coefficient, &Integer::from(4));
                        }
                        [1, 1] => {
                            pairs += 1;
                            assert_eq!(coefficient, &Integer::from(8));
                        }
                        other => panic!("unexpected degree-two exponent support: {other:?}"),
                    }
                }
                assert_eq!(squares, 21);
                assert_eq!(pairs, 210);
            }
            (preflight.backend, preflight.stats)
        };

        // 3^20 fits u32, while 3^21 does not.
        let (safe, safe_stats) = classify(21, 0);
        assert_eq!(
            safe,
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator
        );
        assert_eq!(safe_stats.largest_kronecker_exponent_bits(), 40);
        let (fallback, fallback_stats) = classify(22, 0);
        assert_eq!(
            fallback,
            ResidualAffinePolynomialCompositionBackend::SymbolicaExpressionExpansion
        );
        assert_eq!(fallback_stats.largest_kronecker_exponent_bits(), 42);

        // `heap_pow::to_uni_var` never multiplies the radix of coordinate
        // zero. These paired arity-22 maps both produce 231 terms from 21
        // supported variables, but with coordinate zero free only twenty
        // radices contribute to the u32 stride.
        let (zero_coordinate_free, zero_coordinate_free_stats) = classify(22, 21);
        assert_eq!(
            zero_coordinate_free,
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator
        );
        assert_eq!(
            zero_coordinate_free_stats.largest_kronecker_exponent_bits(),
            42
        );
    }

    #[test]
    fn symbolica_affine_zero_image_still_preflights_later_u16_max_power() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-affine-zero-before-u16-max-power",
            2,
        )
        .unwrap();
        // n0=0, while n1 remains a free identity image.
        let certificate = residual_affine_integer_system_certificate(
            2,
            vec![residual_affine_integer_system_row(
                [0, 1, 0].into_iter().map(Integer::from).collect(),
                0,
            )],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let mut raw = context.template.numerator.zero();
        raw.append_monomial_back(Integer::one(), &[1, u16::MAX]);
        let source = ParametricPolynomial {
            raw,
            context: context.fingerprint.clone(),
        };
        let composed = context
            .compose_polynomial_on_residual_affine_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert!(composed.value().is_zero());
        assert_eq!(composed.stats().power_calls(), 2);
        assert_eq!(composed.stats().native_power_heap_pair_bound(), 1);
        assert_eq!(composed.stats().expanded_contribution_bound(), 0);

        let one_below = ResidualUnitAffinePolynomialCompositionLimits {
            max_native_power_heap_pairs: 0,
            ..ResidualUnitAffinePolynomialCompositionLimits::default()
        };
        assert!(matches!(
            context.preflight_polynomial_on_residual_affine_composition_plan(
                &source, &plan, one_below,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "native power heap pairs",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn symbolica_affine_target_exponent_accepts_65535_and_rejects_65536_typed() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-affine-target-u16-boundary",
            2,
        )
        .unwrap();
        // n0=n1, so target n1 receives the sum of both source exponents.
        let certificate = residual_affine_integer_system_certificate(
            2,
            vec![residual_affine_integer_system_row(
                [0, 1, -1].into_iter().map(Integer::from).collect(),
                0,
            )],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let make_source = |exponents: [u16; 2]| {
            let mut raw = context.template.numerator.zero();
            raw.append_monomial_back(Integer::one(), &exponents);
            ParametricPolynomial {
                raw,
                context: context.fingerprint.clone(),
            }
        };
        let accepted = make_source([u16::MAX, 0]);
        let mapped = context
            .compose_polynomial_on_residual_affine_composition_plan(
                &accepted,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert_eq!(mapped.value().raw.nterms(), 1);
        assert_eq!(mapped.value().raw.exponents(0), &[0, u16::MAX]);

        let rejected = make_source([u16::MAX, 1]);
        assert!(matches!(
            context.preflight_polynomial_on_residual_affine_composition_plan(
                &rejected,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            ),
            Err(ResidualUnitAffineCompositionError::ExponentLimit {
                source_term: 0,
                target_variable: 1,
                requested: 65_536,
                limit: 65_535,
            })
        ));
    }

    #[test]
    fn symbolica_affine_compositor_preserves_gmp_source_and_image_coefficients() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "symbolica-affine-gmp-source-and-image",
            3,
        )
        .unwrap();
        let image_scale = (Integer::one() << 180u32) + Integer::from(3);
        let source_scale = (Integer::one() << 200u32) + Integer::from(7);
        // n0-image_scale*n1-n2=0.
        let certificate = residual_affine_integer_system_certificate(
            3,
            vec![residual_affine_integer_system_row(
                vec![
                    Integer::zero(),
                    Integer::one(),
                    -image_scale.clone(),
                    Integer::from(-1),
                ],
                0,
            )],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let mut raw = context.template.numerator.zero();
        raw.append_monomial_back(source_scale.clone(), &[2, 0, 0]);
        let source = ParametricPolynomial {
            raw,
            context: context.fingerprint.clone(),
        };
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        let native = context
            .compose_polynomial_on_residual_affine_composition_plan(&source, &plan, limits)
            .unwrap();
        assert_eq!(native.value().raw.nterms(), 3);
        let coefficient_at = |target: &[u16]| {
            native
                .value()
                .raw
                .exponents_iter()
                .zip(&native.value().raw.coefficients)
                .find_map(|(exponents, coefficient)| (exponents == target).then_some(coefficient))
                .cloned()
        };
        assert_eq!(
            coefficient_at(&[0, 2, 0]),
            Some(&source_scale * &image_scale.pow(2))
        );
        assert_eq!(
            coefficient_at(&[0, 1, 1]),
            Some(&source_scale * &(&image_scale * 2))
        );
        assert_eq!(coefficient_at(&[0, 0, 2]), Some(source_scale));
    }

    #[test]
    fn source_neutral_affine_coefficient_retains_integer_and_nontrivial_denominators() {
        let context = residual_affine_test_context("source-neutral-coefficient-denominators");
        let plan = residual_affine_integer_plan(&context);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();

        // This denominator depends on the ambient indices but becomes the
        // exact integer 3 on F(n_1,n_2) = (3-n_1,n_1,n_2).
        let integer_denominator = context.add(&n0, &n1).unwrap().raw.numerator;
        let integer_source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: n0.raw.numerator.clone(),
                denominator: integer_denominator,
            },
            context: context.fingerprint.clone(),
        };
        let ResidualAffineCoefficientComposition::Available(integer_mapped) = context
            .compose_coefficient_on_residual_affine_composition_plan(
                &integer_source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap()
        else {
            panic!("a nonzero integer denominator must remain available")
        };
        let expected_integer = residual_affine_polynomial(&context, &context.integer(3));
        assert_eq!(integer_mapped.mapped_denominator(), &expected_integer);
        assert!(integer_mapped.mapped_denominator().is_nonzero_constant());
        assert_eq!(integer_mapped.stats().durable_denominator_terms(), 1);
        assert_eq!(
            integer_mapped.stats().durable_guard_origin_retained_bytes(),
            0
        );
        let (_, retained_integer, integer_stats) = integer_mapped.into_parts();
        assert_eq!(retained_integer, expected_integer);
        assert_eq!(integer_stats.durable_denominator_terms(), 1);

        let denominator = context
            .sub(&context.add(&n0, &n2).unwrap(), &context.integer(2))
            .unwrap()
            .raw
            .numerator;
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: context.template.numerator.one(),
                denominator,
            },
            context: context.fingerprint.clone(),
        };
        let ResidualAffineCoefficientComposition::Available(mapped) = context
            .compose_coefficient_on_residual_affine_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap()
        else {
            panic!("a generically nonzero mapped denominator must remain available")
        };
        let expected = residual_affine_polynomial(
            &context,
            &context
                .add(&context.sub(&context.one(), &n1).unwrap(), &n2)
                .unwrap(),
        );
        assert_eq!(mapped.mapped_denominator(), &expected);
        assert!(!mapped.mapped_denominator().is_nonzero_constant());
        assert_eq!(
            mapped.stats().durable_denominator_terms(),
            mapped.mapped_denominator().term_count()
        );
        assert_eq!(mapped.stats().durable_guard_origin_retained_bytes(), 0);
        assert_eq!(
            mapped.stats().total_integer_bit_work_bound(),
            mapped.stats().aggregate().integer_bit_work_bound()
                + mapped.stats().durable_denominator_integer_bit_payload()
        );
    }

    #[test]
    fn source_neutral_affine_coefficient_retains_denominator_after_cancellation_and_zero_numerator()
    {
        let context = residual_affine_test_context("source-neutral-coefficient-cancellation");
        let plan = residual_affine_integer_plan(&context);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let denominator = context
            .sub(&context.add(&n0, &n2).unwrap(), &context.integer(2))
            .unwrap()
            .raw
            .numerator;
        let expected_denominator = residual_affine_polynomial(
            &context,
            &context
                .add(&context.sub(&context.one(), &n1).unwrap(), &n2)
                .unwrap(),
        );

        for (numerator, expected_value) in [
            (denominator.clone(), context.one()),
            (context.template.numerator.zero(), context.zero()),
        ] {
            let source = ParametricCoefficient {
                raw: RationalPolynomial {
                    numerator,
                    denominator: denominator.clone(),
                },
                context: context.fingerprint.clone(),
            };
            let ResidualAffineCoefficientComposition::Available(mapped) = context
                .compose_coefficient_on_residual_affine_composition_plan(
                    &source,
                    &plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap()
            else {
                panic!("a generically nonzero mapped denominator must remain available")
            };
            assert_eq!(mapped.value(), &expected_value);
            assert_eq!(mapped.mapped_denominator(), &expected_denominator);
            assert!(mapped.stats().normalization_input_term_pairs() > 0);
            assert!(mapped.stats().durable_denominator_terms() > 0);
            assert_eq!(mapped.stats().durable_guard_origin_retained_bytes(), 0);
        }
    }

    #[test]
    fn source_neutral_affine_coefficient_reports_zero_mapped_denominator_with_stats() {
        let context = residual_affine_test_context("source-neutral-coefficient-zero-denominator");
        let plan = residual_affine_integer_plan(&context);
        let equality = context
            .sub(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.integer(3),
            )
            .unwrap();
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: context.template.numerator.one(),
                denominator: equality.raw.numerator,
            },
            context: context.fingerprint.clone(),
        };
        let mapped = context
            .compose_coefficient_on_residual_affine_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            mapped,
            ResidualAffineCoefficientComposition::ZeroMappedDenominator { .. }
        ));
        assert!(mapped.stats().numerator().source_terms() > 0);
        assert!(mapped.stats().denominator().source_terms() > 0);
        assert_eq!(mapped.stats().denominator().output_terms(), 0);
        assert_eq!(mapped.stats().durable_denominator_terms(), 0);
        assert_eq!(mapped.stats().normalization_input_term_pairs(), 0);
    }

    #[test]
    fn source_neutral_affine_coefficient_rejects_wrong_context_and_plan() {
        let context = residual_affine_test_context("source-neutral-coefficient-context");
        let foreign = residual_affine_test_context("source-neutral-coefficient-foreign");
        let plan = residual_affine_integer_plan(&context);
        let foreign_plan = residual_affine_integer_plan(&foreign);
        let source = context.one();
        let foreign_source = foreign.one();
        let limits = ResidualUnitAffinePolynomialCompositionLimits::default();

        assert!(matches!(
            context.compose_coefficient_on_residual_affine_composition_plan(
                &foreign_source,
                &plan,
                limits,
            ),
            Err(ResidualUnitAffineCompositionError::Coefficient(
                ParametricCoefficientError::WrongContext
            ))
        ));
        assert!(matches!(
            context.compose_coefficient_on_residual_affine_composition_plan(
                &source,
                &foreign_plan,
                limits,
            ),
            Err(ResidualUnitAffineCompositionError::WrongContext)
        ));

        let mut malformed_plan = plan.clone();
        malformed_plan.schema = "not-a-residual-affine-composition-schema";
        assert!(matches!(
            context.compose_coefficient_on_residual_affine_composition_plan(
                &source,
                &malformed_plan,
                limits,
            ),
            Err(ResidualUnitAffineCompositionError::SchemaMismatch)
        ));
    }

    #[test]
    fn source_neutral_affine_coefficient_accepts_exact_and_rejects_one_below_budgets() {
        let context = residual_affine_test_context("source-neutral-coefficient-budgets");
        let plan = residual_affine_integer_plan(&context);
        let n0 = context.index(0).unwrap();
        let n2 = context.index(2).unwrap();
        let numerator = context.mul(&n0, &n0).unwrap().raw.numerator;
        let denominator_base = context
            .sub(&context.add(&n0, &n2).unwrap(), &context.integer(2))
            .unwrap();
        let denominator = context
            .mul(&denominator_base, &denominator_base)
            .unwrap()
            .raw
            .numerator;
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator,
                denominator,
            },
            context: context.fingerprint.clone(),
        };
        let preflight = context
            .preflight_coefficient_on_residual_affine_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let reference = context
            .compose_coefficient_on_residual_affine_composition_plan(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let ResidualAffineCoefficientComposition::Available(reference) = reference else {
            panic!("the reference denominator must remain nonzero")
        };
        let stats = reference.stats();
        let aggregate = preflight.aggregate();
        assert!(
            preflight.normalization_input_term_pair_bound()
                >= stats.normalization_input_term_pairs()
        );
        assert!(preflight.total_integer_bit_work_bound() >= stats.total_integer_bit_work_bound());
        let exact = ResidualUnitAffinePolynomialCompositionLimits {
            max_source_terms: aggregate.source_terms(),
            max_source_exponent_entries: aggregate.source_exponent_entries(),
            max_expanded_contributions: aggregate.expanded_contribution_bound(),
            max_output_terms: aggregate.expanded_contribution_bound(),
            max_output_exponent_entries: aggregate.output_exponent_entry_bound(),
            max_power_calls: aggregate.power_calls(),
            max_native_power_heap_pairs: aggregate.native_power_heap_pair_bound(),
            max_multiplication_term_pairs: aggregate.multiplication_term_pair_bound(),
            max_addition_term_visits: aggregate.addition_term_visit_bound(),
            max_kronecker_exponent_bits: aggregate.largest_kronecker_exponent_bits(),
            max_integer_coefficient_bits: aggregate.largest_integer_coefficient_bit_bound(),
            max_integer_bit_work: preflight.total_integer_bit_work_bound(),
            max_normalization_input_term_pairs: preflight.normalization_input_term_pair_bound(),
            max_guard_origins: 0,
            max_guard_origin_retained_bytes: 0,
            ..ResidualUnitAffinePolynomialCompositionLimits::default()
        };
        let ResidualAffineCoefficientComposition::Available(exact_mapped) = context
            .compose_coefficient_on_residual_affine_composition_plan(&source, &plan, exact)
            .unwrap()
        else {
            panic!("every exact resource bound must be accepted")
        };
        assert_eq!(exact_mapped.stats(), stats);
        assert_eq!(
            exact_mapped.mapped_denominator(),
            reference.mapped_denominator()
        );

        macro_rules! reject_one_below {
            ($field:ident, $exact:expr) => {{
                let exact_value = $exact;
                assert!(exact_value > 0, "{} must be exercised", stringify!($field));
                let mut below = exact;
                below.$field = exact_value - 1;
                assert!(matches!(
                    context.compose_coefficient_on_residual_affine_composition_plan(
                        &source, &plan, below,
                    ),
                    Err(ResidualUnitAffineCompositionError::ResourceLimit { .. })
                ));
            }};
        }

        reject_one_below!(max_source_terms, aggregate.source_terms());
        reject_one_below!(
            max_source_exponent_entries,
            aggregate.source_exponent_entries()
        );
        reject_one_below!(
            max_expanded_contributions,
            aggregate.expanded_contribution_bound()
        );
        reject_one_below!(max_output_terms, aggregate.expanded_contribution_bound());
        reject_one_below!(
            max_output_exponent_entries,
            aggregate.output_exponent_entry_bound()
        );
        reject_one_below!(max_power_calls, aggregate.power_calls());
        reject_one_below!(
            max_native_power_heap_pairs,
            aggregate.native_power_heap_pair_bound()
        );
        reject_one_below!(
            max_multiplication_term_pairs,
            aggregate.multiplication_term_pair_bound()
        );
        reject_one_below!(
            max_addition_term_visits,
            aggregate.addition_term_visit_bound()
        );
        reject_one_below!(
            max_kronecker_exponent_bits,
            aggregate.largest_kronecker_exponent_bits()
        );
        reject_one_below!(
            max_integer_coefficient_bits,
            aggregate.largest_integer_coefficient_bit_bound()
        );
        let integer_bit_work_bound = preflight.total_integer_bit_work_bound();
        assert!(integer_bit_work_bound > 0);
        let mut below_integer = exact;
        below_integer.max_integer_bit_work = integer_bit_work_bound - 1;
        assert_eq!(
            context
                .compose_coefficient_on_residual_affine_composition_plan(
                    &source,
                    &plan,
                    below_integer,
                )
                .unwrap_err(),
            ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "coefficient total integer-bit work bound",
                requested: integer_bit_work_bound,
                limit: integer_bit_work_bound - 1,
            }
        );

        let normalization_input_term_pair_bound = preflight.normalization_input_term_pair_bound();
        assert!(normalization_input_term_pair_bound > 0);
        let mut below_normalization = exact;
        below_normalization.max_normalization_input_term_pairs =
            normalization_input_term_pair_bound - 1;
        assert_eq!(
            context
                .compose_coefficient_on_residual_affine_composition_plan(
                    &source,
                    &plan,
                    below_normalization,
                )
                .unwrap_err(),
            ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "coefficient normalization input term-pair bound",
                requested: normalization_input_term_pair_bound,
                limit: normalization_input_term_pair_bound - 1,
            }
        );
    }

    #[test]
    fn ambient_square_adapter_rejects_nonzero_nonfree_column() {
        let constants = vec![Integer::zero(), Integer::zero(), Integer::zero()];
        let linear = vec![
            Integer::one(),
            Integer::from(2),
            Integer::from(3),
            Integer::zero(),
            Integer::one(),
            Integer::zero(),
            Integer::zero(),
            Integer::zero(),
            Integer::one(),
        ];
        let result = compact_ambient_square_affine_geometry_from_accessors(
            3,
            3,
            &[0],
            &[1, 2],
            |row| constants.get(row),
            |row, column| linear.get(row * 3 + column),
            0,
            ResidualUnitAffineCompositionPlanLimits::default(),
        );
        assert!(matches!(
            result,
            Err(ResidualUnitAffineCompositionError::IntegerSystem(
                ResidualAffineIntegerSystemError::ArithmeticInvariantFailure(
                    "ambient affine map has a nonzero nonfree column"
                )
            ))
        ));
    }

    #[test]
    fn integer_system_affine_plan_enforces_exact_aggregate_gmp_bit_limit() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "integer-system-affine-gmp-bits",
            2,
        )
        .unwrap();
        let huge = Integer::from(1) << 4096u32;
        let certificate = residual_affine_integer_system_certificate(
            2,
            vec![residual_affine_integer_system_row(
                vec![huge, Integer::from(-1), Integer::from(3)],
                0,
            )],
        );
        let permissive = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate.clone(),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let exact = permissive.stats().total_image_integer_bits();
        assert_eq!(exact, 4_100);
        assert_eq!(permissive.stats().largest_image_integer_bits(), 4_097);

        let exact_limits = ResidualUnitAffineCompositionPlanLimits {
            max_total_image_integer_bits: exact,
            ..ResidualUnitAffineCompositionPlanLimits::default()
        };
        let exact_plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate.clone(),
                exact_limits,
            )
            .unwrap();
        assert_eq!(exact_plan.stats().total_image_integer_bits(), exact);

        let below_limits = ResidualUnitAffineCompositionPlanLimits {
            max_total_image_integer_bits: exact - 1,
            ..ResidualUnitAffineCompositionPlanLimits::default()
        };
        assert!(matches!(
            context.compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                below_limits,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "total image integer bits",
                requested: 4_100,
                limit: 4_099,
            })
        ));
    }

    #[test]
    fn affine_plan_logical_memory_is_exact_capacity_independent_and_multi_large() {
        fn large_dynamic_bytes(value: &Integer) -> usize {
            match value {
                Integer::Large(value) => {
                    (usize::try_from(value.significant_bits()).unwrap() + 7) / 8
                        + size_of::<usize>()
                }
                Integer::Single(_) | Integer::Double(_) => 0,
            }
        }

        fn polynomial_dynamic_bytes(value: &CoefficientPolynomial) -> usize {
            value.coefficients.len() * size_of::<Integer>()
                + value.exponents.len() * size_of::<u16>()
                + value
                    .coefficients
                    .iter()
                    .map(large_dynamic_bytes)
                    .sum::<usize>()
        }

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "integer-system-affine-plan-logical-memory",
            3,
        )
        .unwrap();
        let huge_constant = (Integer::from(1) << 256u32) + Integer::from(1);
        let huge_linear = (Integer::from(1) << 320u32) + Integer::from(3);
        let certificate = residual_affine_integer_system_certificate(
            3,
            vec![residual_affine_integer_system_row(
                vec![
                    huge_constant,
                    Integer::from(-1),
                    huge_linear,
                    Integer::zero(),
                ],
                0,
            )],
        );
        let mut plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let census = plan.recompute_logical_memory_census().unwrap();
        assert_eq!(census, plan.recompute_logical_memory_census().unwrap());
        assert!(
            plan.core
                .full_images
                .iter()
                .flat_map(|image| &image.coefficients)
                .filter(|coefficient| matches!(coefficient, Integer::Large(_)))
                .count()
                >= 2
        );

        let full_image_dynamic_bytes = plan
            .core
            .full_images
            .iter()
            .map(polynomial_dynamic_bytes)
            .sum::<usize>();
        let linear_support_bytes =
            (plan.core.linear_support.len() + u8::BITS as usize - 1) / u8::BITS as usize;
        let expected_retained = size_of::<ResidualAffineCompositionPlan>()
            + arc_payload_control_and_padding_byte_bound::<ResidualAffineCompositionCorePlan>()
                .unwrap()
            + plan.core.free_positions.len() * size_of::<usize>()
            + plan.core.nonfree_positions.len() * size_of::<usize>()
            + linear_support_bytes
            + plan.core.full_images.len() * size_of::<CoefficientPolynomial>()
            + full_image_dynamic_bytes
            + plan.core.image_term_counts.len() * size_of::<usize>()
            + plan.core.image_coefficient_growth_bits.len() * size_of::<usize>();
        assert_eq!(census.retained_owned_logical_bytes(), expected_retained);

        let compact_geometry_large_bytes = plan
            .core
            .full_images
            .iter()
            .flat_map(|image| &image.coefficients)
            .map(large_dynamic_bytes)
            .sum::<usize>();
        let compact_geometry_bytes = size_of::<ResidualAffineCompactGeometry>()
            + plan.core.ambient_arity * size_of::<usize>()
            + plan.stats.geometry_entries_retained() * size_of::<Integer>()
            + compact_geometry_large_bytes;
        let exponent_scratch_bytes =
            size_of::<Vec<u16>>() + plan.core.full_images.len() * size_of::<u16>();
        let largest_current_image = plan
            .core
            .full_images
            .iter()
            .map(|image| size_of::<CoefficientPolynomial>() + polynomial_dynamic_bytes(image))
            .max()
            .unwrap();
        let expected_peak = expected_retained
            + compact_geometry_bytes
            + exponent_scratch_bytes
            + largest_current_image;
        assert_eq!(
            census.compilation_owned_logical_peak_upper_bound(),
            expected_peak
        );

        let exact_retained_limit = expected_retained;
        let exact_peak_limit = expected_peak;
        assert!(census.retained_owned_logical_bytes() <= exact_retained_limit);
        assert!(census.compilation_owned_logical_peak_upper_bound() <= exact_peak_limit);
        assert!(census.retained_owned_logical_bytes() > exact_retained_limit - 1);
        assert!(census.compilation_owned_logical_peak_upper_bound() > exact_peak_limit - 1);

        let legacy_capacity_before = plan.owned_retained_byte_bound().unwrap();
        let core = Arc::make_mut(&mut plan.core);
        core.free_positions.reserve(64);
        core.nonfree_positions.reserve(64);
        core.linear_support.reserve(64);
        core.full_images.reserve(64);
        core.image_term_counts.reserve(64);
        core.image_coefficient_growth_bits.reserve(64);
        for image in &mut core.full_images {
            image.coefficients.reserve(64);
            image.exponents.reserve(64);
        }
        assert_eq!(plan.recompute_logical_memory_census().unwrap(), census);
        assert!(plan.owned_retained_byte_bound().unwrap() > legacy_capacity_before);
    }

    #[test]
    fn affine_plan_limit_memory_envelope_is_exact_checked_and_dominates_concrete_plan() {
        let zero = ResidualUnitAffineCompositionPlanLimits {
            max_variables: 0,
            max_full_images: 0,
            max_geometry_entries_inspected: 0,
            max_geometry_entries_retained: 0,
            max_support_entries_retained: 0,
            max_total_image_terms: 0,
            max_total_image_exponent_entries: 0,
            max_image_integer_bits: 0,
            max_total_image_integer_bits: 0,
        };
        let zero_envelope =
            residual_affine_composition_plan_memory_envelope_from_limits(zero).unwrap();
        let expected_retained = size_of::<ResidualAffineCompositionPlan>()
            + arc_payload_control_and_padding_byte_bound::<ResidualAffineCompositionCorePlan>()
                .unwrap();
        assert_eq!(
            zero_envelope.retained_owned_logical_bytes(),
            expected_retained
        );
        assert_eq!(
            zero_envelope.compilation_owned_logical_peak_upper_bound(),
            expected_retained
                + size_of::<ResidualAffineCompactGeometry>()
                + size_of::<Vec<u16>>()
                + size_of::<CoefficientPolynomial>()
        );

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["theta"]),
            "plan-limit-memory-envelope",
            2,
        )
        .unwrap();
        let certificate = residual_affine_integer_system_certificate(
            2,
            vec![residual_affine_integer_system_row(
                vec![Integer::from(3), Integer::from(-1), Integer::from(2)],
                0,
            )],
        );
        let plan = context
            .compile_residual_affine_composition_plan_from_integer_system(
                certificate,
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        let concrete = plan.recompute_logical_memory_census().unwrap();
        let envelope = residual_affine_composition_plan_memory_envelope_from_limits(
            ResidualUnitAffineCompositionPlanLimits::default(),
        )
        .unwrap();
        assert!(concrete.retained_owned_logical_bytes() <= envelope.retained_owned_logical_bytes());
        assert!(
            concrete.compilation_owned_logical_peak_upper_bound()
                <= envelope.compilation_owned_logical_peak_upper_bound()
        );

        let overflow = ResidualUnitAffineCompositionPlanLimits {
            max_total_image_terms: usize::MAX,
            ..zero
        };
        assert!(matches!(
            residual_affine_composition_plan_memory_envelope_from_limits(overflow),
            Err(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "affine composition plan memory envelope"
            })
        ));
    }

    #[test]
    fn affine_geometry_adapters_accept_exact_unit_coefficient_bit_limit() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "affine-base-identity-unit-bits-exact",
            3,
        )
        .unwrap();
        let legacy_map = residual_affine_all_pivot_zero_legacy_map(&context);
        let limits = ResidualUnitAffineCompositionPlanLimits {
            max_image_integer_bits: 1,
            ..ResidualUnitAffineCompositionPlanLimits::default()
        };
        let legacy = compact_legacy_unit_affine_geometry(
            &legacy_map,
            context.index_count(),
            context.base.variables().len(),
            limits,
        )
        .unwrap();
        assert_eq!(legacy.largest_image_integer_bits, 1);
        assert_eq!(legacy.total_image_integer_bits, 1);

        let certificate = residual_affine_integer_system_certificate(
            1,
            vec![residual_affine_integer_system_row(
                vec![Integer::zero(), Integer::one()],
                0,
            )],
        );
        let integer_map = certificate.affine_map().unwrap();
        assert!(integer_map.free_positions().is_empty());
        assert_eq!(integer_map.constant(0), Some(&Integer::zero()));
        let integer = compact_integer_system_affine_geometry(integer_map, 1, 1, limits).unwrap();
        assert_eq!(integer.largest_image_integer_bits, 1);
        assert_eq!(integer.total_image_integer_bits, 1);
    }

    #[test]
    fn affine_geometry_adapters_reject_one_below_unit_coefficient_bit_limit() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["d"]),
            "affine-base-identity-unit-bits-below",
            3,
        )
        .unwrap();
        let legacy_map = residual_affine_all_pivot_zero_legacy_map(&context);
        let limits = ResidualUnitAffineCompositionPlanLimits {
            max_image_integer_bits: 0,
            ..ResidualUnitAffineCompositionPlanLimits::default()
        };
        assert!(matches!(
            compact_legacy_unit_affine_geometry(
                &legacy_map,
                context.index_count(),
                context.base.variables().len(),
                limits,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "image integer coefficient bits",
                requested: 1,
                limit: 0,
            })
        ));

        let certificate = residual_affine_integer_system_certificate(
            1,
            vec![residual_affine_integer_system_row(
                vec![Integer::zero(), Integer::one()],
                0,
            )],
        );
        let integer_map = certificate.affine_map().unwrap();
        assert!(integer_map.free_positions().is_empty());
        assert!(matches!(
            compact_integer_system_affine_geometry(integer_map, 1, 1, limits),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "image integer coefficient bits",
                requested: 1,
                limit: 0,
            })
        ));
    }

    #[test]
    fn residual_affine_composition_is_simultaneous_and_removes_bound_positions() {
        let context = residual_affine_test_context("affine-compose-simultaneous");
        let plan = residual_affine_plan(&context, true);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let source = context
            .add(
                &context.mul(&n0, &n1).unwrap(),
                &context.mul(&n2, &n0).unwrap(),
            )
            .unwrap();
        let source = residual_affine_polynomial(&context, &source);
        let mapped = context
            .compose_polynomial_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();

        let expected = context
            .mul(
                &context.sub(&context.integer(3), &n1).unwrap(),
                &context.add(&n1, &context.integer(2)).unwrap(),
            )
            .unwrap();
        assert_eq!(
            mapped.value(),
            &residual_affine_polynomial(&context, &expected)
        );
        let base_count = context.base.variables().len();
        assert!(
            mapped
                .value
                .raw
                .exponents_iter()
                .all(|exponents| { exponents[base_count] == 0 && exponents[base_count + 2] == 0 })
        );
        assert!(mapped.stats().expanded_contribution_bound() >= mapped.stats().output_terms());
    }

    #[test]
    fn residual_affine_native_full_point_matches_safe_sequential_oracle() {
        let context = residual_affine_test_context("affine-compose-sequential-oracle");
        let plan = residual_affine_plan(&context, true);
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n2 = context.index(2).unwrap();
        let source = context
            .mul(
                &context.add(&n0, &n2).unwrap(),
                &context.add(&n0, &n1).unwrap(),
            )
            .unwrap();
        let source = residual_affine_polynomial(&context, &source);
        let mapped = context
            .compose_polynomial_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();

        let base_count = context.base.variables().len();
        let mut sequential = source.raw.clone();
        // V1 images mention only unchanged free variables, so sequential
        // replacement is an independent oracle (not the production path).
        sequential = sequential.replace_with_poly(base_count, &plan.core.full_images[base_count]);
        sequential =
            sequential.replace_with_poly(base_count + 2, &plan.core.full_images[base_count + 2]);
        assert_eq!(mapped.value.raw, sequential);
    }

    #[test]
    fn residual_affine_supports_overlapping_free_images_and_preflights_backend_radix() {
        let context = residual_affine_test_context("affine-compose-overlapping-free-images");
        let sum = context
            .add(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.index(2).unwrap(),
            )
            .unwrap();
        let affine = context.sub(&sum, &context.integer(3)).unwrap();
        let plan = context
            .compile_residual_unit_affine_composition_plan(
                residual_affine_test_map_for_predicate(&context, affine, false),
                ResidualUnitAffineCompositionPlanLimits::default(),
            )
            .unwrap();
        assert_eq!(plan.map().free_positions(), &[1, 2]);

        let n0_squared = context
            .mul(&context.index(0).unwrap(), &context.index(0).unwrap())
            .unwrap();
        let mapped = context
            .compose_polynomial_on_residual_unit_affine_map(
                &residual_affine_polynomial(&context, &n0_squared),
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let expected_image = context
            .sub(
                &context
                    .sub(&context.integer(3), &context.index(1).unwrap())
                    .unwrap(),
                &context.index(2).unwrap(),
            )
            .unwrap();
        let expected = context.mul(&expected_image, &expected_image).unwrap();
        assert_eq!(
            mapped.value(),
            &residual_affine_polynomial(&context, &expected)
        );

        let base_count = context.base.variables().len();
        let source_power = |exponent: u16| {
            let mut raw = context.template.numerator.zero_with_capacity(1);
            let mut exponents = vec![0u16; context.variables.len()];
            exponents[base_count] = exponent;
            raw.append_monomial(Integer::one(), &exponents);
            ParametricPolynomial {
                raw,
                context: context.fingerprint.clone(),
            }
        };
        let maximal = source_power(u16::MAX);
        let wide_limits = ResidualUnitAffinePolynomialCompositionLimits {
            exact_algebra: ExactAlgebraLimits {
                max_polynomial_terms: usize::MAX,
                max_term_operations: usize::MAX,
                ..ExactAlgebraLimits::default()
            },
            max_expanded_contributions: usize::MAX,
            max_output_terms: usize::MAX,
            max_output_exponent_entries: usize::MAX,
            max_native_power_heap_pairs: usize::MAX,
            max_multiplication_term_pairs: usize::MAX,
            max_addition_term_visits: usize::MAX,
            max_integer_coefficient_bits: usize::MAX,
            max_integer_bit_work: usize::MAX,
            ..ResidualUnitAffinePolynomialCompositionLimits::default()
        };

        // The exact vendored evaluator stride is (e+1)^2 here. At u16::MAX
        // that is one larger than u32::MAX, so backend classification selects
        // expression expansion. Its honest H*U*B integer-work census exceeds
        // usize and must reject before Symbolica rather than wrap.
        let maximal_radix = residual_affine_kronecker_preflight(
            &plan.core,
            base_count,
            usize::from(u16::MAX),
            base_count,
        )
        .unwrap();
        assert!(!maximal_radix.polynomial_evaluator_safe);
        assert_eq!(maximal_radix.exponent_bits, 32);
        assert!(matches!(
            context.preflight_residual_affine_polynomial_core(&maximal, &plan.core, wide_limits,),
            Err(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "Symbolica expression integer bit work",
            })
        ));

        // A still-large feasible power exercises the same 32-bit policy
        // boundary while remaining below the evaluator's exact u32 stride.
        let feasible = source_power(32_768);
        let preflight = context
            .preflight_residual_affine_polynomial_core(&feasible, &plan.core, wide_limits)
            .unwrap();
        assert_eq!(
            preflight.backend,
            ResidualAffinePolynomialCompositionBackend::PolynomialEvaluator
        );
        assert_eq!(preflight.stats.largest_kronecker_exponent_bits(), 32);

        let strict = ResidualUnitAffinePolynomialCompositionLimits {
            max_kronecker_exponent_bits: 31,
            ..wide_limits
        };
        assert!(matches!(
            context.preflight_residual_affine_polynomial_core(&maximal, &plan.core, strict),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "Kronecker exponent bits",
                requested: 32,
                limit: 31,
            })
        ));
    }

    #[test]
    fn residual_affine_mapped_zero_guard_is_semantic_and_retains_stats() {
        let context = residual_affine_test_context("affine-compose-zero-guard");
        let plan = residual_affine_plan(&context, false);
        let equality = context
            .sub(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.integer(3),
            )
            .unwrap();
        let source = context
            .nonzero_condition(
                residual_affine_polynomial(&context, &equality),
                GuardOrigin::ExplicitRelationCondition,
            )
            .unwrap();
        let mapped = context
            .compose_nonzero_condition_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            mapped.class(),
            ResidualUnitAffineConditionClass::Unsatisfiable
        ));
        assert!(mapped.stats().source_terms() > 0);
        assert_eq!(mapped.stats().output_terms(), 0);
        assert_eq!(mapped.guard_origin_retained_bytes(), 0);
    }

    #[test]
    fn residual_affine_mapped_zero_denominator_is_semantic_and_retains_stats() {
        let context = residual_affine_test_context("affine-compose-zero-denominator");
        let plan = residual_affine_plan(&context, false);
        let equality = context
            .sub(
                &context
                    .add(&context.index(0).unwrap(), &context.index(1).unwrap())
                    .unwrap(),
                &context.integer(3),
            )
            .unwrap();
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: context.template.numerator.one(),
                denominator: equality.raw.numerator,
            },
            context: context.fingerprint.clone(),
        };
        let mapped = context
            .compose_coefficient_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert!(matches!(
            mapped,
            ResidualUnitAffineCoefficientComposition::ZeroMappedDenominator { .. }
        ));
        assert!(mapped.stats().aggregate().source_terms() > 0);
    }

    #[test]
    fn residual_affine_retains_denominator_after_cancellation_and_zero_numerator() {
        let context = residual_affine_test_context("affine-compose-denominator-retention");
        let plan = residual_affine_plan(&context, false);
        let denominator = context
            .sub(
                &context
                    .add(&context.index(0).unwrap(), &context.index(2).unwrap())
                    .unwrap(),
                &context.integer(2),
            )
            .unwrap()
            .raw
            .numerator;

        for numerator in [denominator.clone(), context.template.numerator.zero()] {
            let source = ParametricCoefficient {
                raw: RationalPolynomial {
                    numerator,
                    denominator: denominator.clone(),
                },
                context: context.fingerprint.clone(),
            };
            let mapped = context
                .compose_coefficient_on_residual_unit_affine_map(
                    &source,
                    &plan,
                    ResidualUnitAffinePolynomialCompositionLimits::default(),
                )
                .unwrap();
            let ResidualUnitAffineCoefficientComposition::Available(mapped) = mapped else {
                panic!("generic denominator must remain available on the locus")
            };
            assert!(mapped.mapped_denominator().is_some());
            assert!(mapped.stats().normalization_input_term_pairs() > 0);
            assert!(mapped.stats().durable_guard_origin_retained_bytes() > 0);
            assert_eq!(
                mapped.stats().total_integer_bit_work_bound(),
                mapped.stats().aggregate().integer_bit_work_bound()
                    + mapped.stats().durable_guard_integer_bit_payload()
            );
            assert!(
                mapped.stats().numerator().source_terms()
                    <= mapped.stats().aggregate().source_terms()
            );
            assert!(mapped.stats().denominator().source_terms() > 0);
            let origins = mapped.mapped_denominator().unwrap().origins();
            assert!(origins.iter().any(|origin| matches!(
                origin,
                GuardOrigin::CoefficientResidualUnitAffineSubstitutionDenominator { .. }
            )));
            assert!(origins.iter().any(|origin| matches!(
                origin,
                GuardOrigin::ResidualUnitAffineIndexSubstitution { .. }
            )));
        }
    }

    #[test]
    fn residual_affine_classifies_base_and_index_dependent_guards() {
        let context = residual_affine_test_context("affine-compose-guard-classes");
        let plan = residual_affine_plan(&context, false);
        let d = context
            .lift(&context.base().parameter("d").unwrap())
            .unwrap();
        let base_guard = context
            .nonzero_condition(
                residual_affine_polynomial(&context, &context.add(&d, &context.one()).unwrap()),
                GuardOrigin::ExplicitRelationCondition,
            )
            .unwrap();
        let mapped_base = context
            .compose_nonzero_condition_on_residual_unit_affine_map(
                &base_guard,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let ResidualUnitAffineConditionClass::BaseAssumption(condition) = mapped_base.class()
        else {
            panic!("base-only guard must become an affine-locus base assumption")
        };
        assert!(
            !context
                .polynomial_depends_on_indices(condition.polynomial())
                .unwrap()
        );
        assert!(mapped_base.guard_origin_retained_bytes() > 0);

        let index_guard = context
            .nonzero_condition(
                residual_affine_polynomial(&context, &context.index(1).unwrap()),
                GuardOrigin::ExplicitRelationCondition,
            )
            .unwrap();
        let mapped_index = context
            .compose_nonzero_condition_on_residual_unit_affine_map(
                &index_guard,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let ResidualUnitAffineConditionClass::IndexDependent(condition) = mapped_index.class()
        else {
            panic!("free-index guard must remain index dependent")
        };
        assert!(condition.origins().iter().any(|origin| matches!(
            origin,
            GuardOrigin::ResidualUnitAffineIndexSubstitution { .. }
        )));
        assert!(mapped_index.guard_origin_retained_bytes() > 0);

        let mut strict = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict.max_guard_origin_retained_bytes = mapped_index.guard_origin_retained_bytes() - 1;
        assert!(matches!(
            context.compose_nonzero_condition_on_residual_unit_affine_map(
                &index_guard,
                &plan,
                strict,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "unit-affine guard origin retained bytes",
                requested,
                limit,
            }) if requested == mapped_index.guard_origin_retained_bytes()
                && limit + 1 == mapped_index.guard_origin_retained_bytes()
        ));
    }

    #[test]
    fn residual_affine_preflights_expansion_exponent_and_integer_bits() {
        let context = residual_affine_test_context("affine-compose-preflights");
        let plan = residual_affine_plan(&context, false);
        let base_count = context.base.variables().len();

        let mut n0_pow_4 = context.template.numerator.zero_with_capacity(1);
        let mut exponents = vec![0u16; context.variables.len()];
        exponents[base_count] = 4;
        n0_pow_4.append_monomial(Integer::one(), &exponents);
        let source = ParametricPolynomial {
            raw: n0_pow_4,
            context: context.fingerprint.clone(),
        };
        let mut limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        limits.max_expanded_contributions = 4;
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(&source, &plan, limits),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "affine power terms",
                requested: 5,
                limit: 4,
            })
        ));

        let mut overflowing = context.template.numerator.zero_with_capacity(1);
        exponents.fill(0);
        exponents[0] = 12;
        exponents[base_count] = 1;
        exponents[base_count + 1] = u16::MAX;
        overflowing.append_monomial(Integer::one(), &exponents);
        let overflowing = ParametricPolynomial {
            raw: overflowing,
            context: context.fingerprint.clone(),
        };
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(
                &overflowing,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            ),
            Err(ResidualUnitAffineCompositionError::ExponentLimit {
                requested: 65_536,
                ..
            })
        ));

        let mut limits = ResidualUnitAffinePolynomialCompositionLimits::default();
        limits.max_integer_coefficient_bits = 3;
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(&source, &plan, limits),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer coefficient bits",
                ..
            })
        ));
    }

    #[test]
    fn residual_affine_integer_bit_work_charges_global_collision_growth() {
        let context = residual_affine_test_context("affine-compose-collision-bit-work");
        let plan = residual_affine_plan(&context, false);
        let source = context
            .add(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap();
        let source = residual_affine_polynomial(&context, &source);

        let generous = context
            .compose_polynomial_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let stats = generous.stats();
        assert_eq!(stats.expanded_contribution_bound(), 3);
        assert_eq!(stats.output_exponent_entry_bound(), 15);
        assert_eq!(stats.output_exponent_entries(), 5);
        assert!(stats.largest_integer_coefficient_bit_bound() >= 4);
        assert_eq!(
            stats.integer_bit_work_bound(),
            stats.native_integer_bit_work_bound()
                + stats.expanded_contribution_bound()
                    * stats.largest_integer_coefficient_bit_bound()
        );

        let mut strict = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict.max_integer_bit_work = stats.integer_bit_work_bound() - 1;
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(&source, &plan, strict),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer bit work",
                requested,
                limit,
            }) if requested == stats.integer_bit_work_bound()
                && limit + 1 == stats.integer_bit_work_bound()
        ));
    }

    #[test]
    fn residual_affine_zero_image_still_charges_source_and_later_native_power_integer_work() {
        let context = residual_affine_test_context("affine-compose-zero-image-native-work");
        let plan = residual_affine_zero_n0_bound_n2_plan(&context);
        assert_eq!(
            plan.core.image_term_counts[context.base.variables().len()],
            0
        );

        let coefficient = Integer::from(2).pow(256);
        let coefficient_bits = residual_affine_integer_bits(&coefficient).unwrap();
        let mut raw = context.template.numerator.zero_with_capacity(1);
        let mut exponents = vec![0u16; context.variables.len()];
        let base_count = context.base.variables().len();
        exponents[base_count] = 1;
        exponents[base_count + 2] = 12;
        raw.append_monomial(coefficient, &exponents);
        let source = ParametricPolynomial {
            raw,
            context: context.fingerprint.clone(),
        };

        let mapped = context
            .compose_polynomial_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let stats = mapped.stats();
        assert!(mapped.value().is_zero());
        assert_eq!(stats.expanded_contribution_bound(), 0);
        assert_eq!(stats.output_terms(), 0);
        assert_eq!(stats.power_calls(), 2);
        assert!(stats.native_integer_bit_work_bound() > coefficient_bits);
        assert_eq!(
            stats.integer_bit_work_bound(),
            stats.native_integer_bit_work_bound()
        );

        let mut strict_bits = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict_bits.max_integer_coefficient_bits = coefficient_bits - 1;
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(
                &source,
                &plan,
                strict_bits,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer coefficient bits",
                requested,
                limit,
            }) if requested == coefficient_bits && limit + 1 == coefficient_bits
        ));

        let mut affine_power_only = context.template.numerator.zero_with_capacity(1);
        exponents.fill(0);
        exponents[base_count + 2] = 12;
        affine_power_only.append_monomial(Integer::one(), &exponents);
        let affine_power_only = ParametricPolynomial {
            raw: affine_power_only,
            context: context.fingerprint.clone(),
        };
        let mut strict_heap_temporary = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict_heap_temporary.max_integer_coefficient_bits = 40;
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(
                &affine_power_only,
                &plan,
                strict_heap_temporary,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer coefficient bits",
                requested,
                limit: 40,
            }) if requested > 40
        ));

        let mut strict_work = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict_work.max_integer_bit_work = stats.integer_bit_work_bound() - 1;
        assert!(matches!(
            context.compose_polynomial_on_residual_unit_affine_map(
                &source,
                &plan,
                strict_work,
            ),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "integer bit work",
                requested,
                limit,
            }) if requested == stats.integer_bit_work_bound()
                && limit + 1 == stats.integer_bit_work_bound()
        ));
    }

    #[test]
    fn residual_affine_exponent_entry_bound_aggregates_and_limits_both_rational_halves() {
        let context = residual_affine_test_context("affine-compose-exponent-entry-budget");
        let plan = residual_affine_plan(&context, false);
        let numerator = context
            .add(&context.index(0).unwrap(), &context.index(1).unwrap())
            .unwrap()
            .raw
            .numerator;
        let denominator = context
            .add(&context.index(1).unwrap(), &context.index(2).unwrap())
            .unwrap()
            .raw
            .numerator;
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator,
                denominator,
            },
            context: context.fingerprint.clone(),
        };

        let mapped = context
            .compose_coefficient_on_residual_unit_affine_map(
                &source,
                &plan,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let ResidualUnitAffineCoefficientComposition::Available(mapped) = mapped else {
            panic!("nonzero free-index denominator should remain available")
        };
        let stats = mapped.stats();
        assert_eq!(stats.numerator().output_exponent_entry_bound(), 15);
        assert_eq!(stats.numerator().output_exponent_entries(), 5);
        assert_eq!(stats.denominator().output_exponent_entry_bound(), 10);
        assert_eq!(stats.denominator().output_exponent_entries(), 10);
        assert_eq!(stats.aggregate().output_exponent_entry_bound(), 25);
        assert_eq!(stats.aggregate().output_exponent_entries(), 15);

        let mut strict = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict.max_output_exponent_entries = 24;
        assert!(matches!(
            context.compose_coefficient_on_residual_unit_affine_map(&source, &plan, strict),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "prospective output exponent entries",
                requested: 10,
                limit: 9,
            })
        ));
    }

    #[test]
    fn residual_affine_uncapped_binomial_overflow_is_a_count_error() {
        assert!(matches!(
            residual_affine_affine_power_term_bound(u16::MAX as usize, 6, usize::MAX),
            Err(ResidualUnitAffineCompositionError::ResourceCountOverflow {
                resource: "affine power terms"
            })
        ));
    }

    #[test]
    fn residual_affine_rejects_foreign_context_before_plan_allocation() {
        let context = residual_affine_test_context("affine-compose-own-context");
        let foreign = residual_affine_test_context("affine-compose-foreign-context");
        let map = residual_affine_test_map(&foreign, false);
        assert!(matches!(
            context.compile_residual_unit_affine_composition_plan(
                map,
                ResidualUnitAffineCompositionPlanLimits::default(),
            ),
            Err(ResidualUnitAffineCompositionError::WrongContext)
        ));
    }

    #[test]
    fn residual_affine_zero_numerator_normalization_uses_dedicated_limit_and_separate_stats() {
        let context = residual_affine_test_context("affine-compose-normalization-limit");
        let plan = residual_affine_plan(&context, false);
        let denominator = context
            .add(&context.index(1).unwrap(), &context.index(2).unwrap())
            .unwrap()
            .raw
            .numerator;
        let source = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: context.template.numerator.zero(),
                denominator,
            },
            context: context.fingerprint.clone(),
        };

        let preflight = context
            .preflight_residual_affine_coefficient_core(
                &source,
                &plan.core,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        let normalization_input_term_pair_bound = preflight.normalization_input_term_pair_bound();
        assert!(normalization_input_term_pair_bound > 0);
        let mut exact = ResidualUnitAffinePolynomialCompositionLimits::default();
        exact.max_normalization_input_term_pairs = normalization_input_term_pair_bound;
        exact.max_integer_bit_work = preflight.total_integer_bit_work_bound();

        let mut strict = exact;
        strict.max_normalization_input_term_pairs = normalization_input_term_pair_bound - 1;
        assert_eq!(
            context
                .compose_coefficient_on_residual_unit_affine_map(&source, &plan, strict)
                .unwrap_err(),
            ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "coefficient normalization input term-pair bound",
                requested: normalization_input_term_pair_bound,
                limit: normalization_input_term_pair_bound - 1,
            }
        );

        let mapped = context
            .compose_coefficient_on_residual_unit_affine_map(&source, &plan, exact)
            .unwrap();
        let ResidualUnitAffineCoefficientComposition::Available(mapped) = mapped else {
            panic!("generic denominator should remain available")
        };
        let stats = mapped.stats();
        assert_eq!(stats.numerator().output_terms(), 0);
        assert_eq!(stats.denominator().output_terms(), 2);
        assert_eq!(stats.normalization_input_term_pairs(), 2);
        assert_eq!(
            preflight.normalization_input_term_pair_bound(),
            stats.normalization_input_term_pairs()
        );
        assert!(stats.durable_guard_integer_bit_payload() > 0);
        assert!(stats.durable_guard_origin_retained_bytes() > 0);
        assert_eq!(
            stats.total_integer_bit_work_bound(),
            stats.aggregate().integer_bit_work_bound() + stats.durable_guard_integer_bit_payload()
        );
        assert_eq!(
            stats.aggregate().source_terms(),
            stats.numerator().source_terms() + stats.denominator().source_terms()
        );

        let integer_bit_work_bound = preflight.total_integer_bit_work_bound();
        assert!(integer_bit_work_bound > 0);
        let mut strict_copy = exact;
        strict_copy.max_integer_bit_work = integer_bit_work_bound - 1;
        assert_eq!(
            context
                .compose_coefficient_on_residual_unit_affine_map(&source, &plan, strict_copy)
                .unwrap_err(),
            ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "coefficient total integer-bit work bound",
                requested: integer_bit_work_bound,
                limit: integer_bit_work_bound - 1,
            }
        );

        let mut strict_origins = exact;
        strict_origins.max_guard_origin_retained_bytes =
            stats.durable_guard_origin_retained_bytes() - 1;
        assert!(matches!(
            context.compose_coefficient_on_residual_unit_affine_map(&source, &plan, strict_origins),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "unit-affine coefficient denominator guard origin retained bytes",
                requested,
                limit,
            }) if requested == stats.durable_guard_origin_retained_bytes()
                && limit + 1 == stats.durable_guard_origin_retained_bytes()
        ));
    }

    #[test]
    fn residual_affine_durable_guard_copy_checks_shape_before_reserved_sparse_copy() {
        let context = residual_affine_test_context("affine-compose-durable-guard-copy");
        let polynomial = residual_affine_polynomial(
            &context,
            &context
                .add(&context.index(1).unwrap(), &context.index(2).unwrap())
                .unwrap(),
        );
        let mut strict = ResidualUnitAffinePolynomialCompositionLimits::default();
        strict.max_output_terms = polynomial.raw.nterms() - 1;
        assert!(matches!(
            context.copy_residual_unit_affine_guard_polynomial(&polynomial, strict),
            Err(ResidualUnitAffineCompositionError::ResourceLimit {
                resource: "durable denominator terms",
                requested: 2,
                limit: 1,
            })
        ));

        let copied = context
            .copy_residual_unit_affine_guard_polynomial(
                &polynomial,
                ResidualUnitAffinePolynomialCompositionLimits::default(),
            )
            .unwrap();
        assert_eq!(copied.value, polynomial);
        assert_eq!(copied.terms, 2);
        assert_eq!(copied.exponent_entries, 10);
        assert!(copied.integer_bit_payload > 0);
        assert!(Arc::ptr_eq(&copied.value.raw.variables, &context.variables));
    }

    #[test]
    fn base_field_may_be_q_and_indices_remain_distinct() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "empty-base", 2).unwrap();
        assert_eq!(base.parameter_names(), &[] as &[String]);
        assert_eq!(context.index_count(), 2);
        assert!(context.contains(&context.index(0).unwrap()));
    }

    #[test]
    fn prevalidated_condition_copies_preserve_exact_owned_payload_and_shared_identity() {
        let base = CoefficientContext::new(["theta"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "fallible-condition-copy", 2).unwrap();
        let numerator = context
            .add(&context.index(0).unwrap(), &context.integer(1))
            .unwrap();
        let denominator = context
            .add(&context.index(1).unwrap(), &context.integer(2))
            .unwrap();
        let value = context.checked_div(&numerator, &denominator).unwrap();
        context
            .validate_with_limits(&value, ExactAlgebraLimits::default())
            .unwrap();
        assert!(value.raw.numerator.nterms() > 1);
        assert!(value.raw.denominator.nterms() > 1);

        let copied_numerator = value.try_copy_prevalidated_numerator_condition().unwrap();
        let copied_denominator = value.try_copy_prevalidated_denominator_condition().unwrap();

        assert_eq!(copied_numerator.raw(), &value.raw.numerator);
        assert_eq!(copied_denominator.raw(), &value.raw.denominator);
        assert!(context.contains_polynomial(&copied_numerator));
        assert!(context.contains_polynomial(&copied_denominator));
        assert!(Arc::ptr_eq(&copied_numerator.context, &value.context));
        assert!(Arc::ptr_eq(&copied_denominator.context, &value.context));
        assert!(Arc::ptr_eq(
            &copied_numerator.raw.variables,
            &value.raw.numerator.variables,
        ));
        assert!(Arc::ptr_eq(
            &copied_denominator.raw.variables,
            &value.raw.denominator.variables,
        ));
        assert_ne!(
            copied_numerator.raw.coefficients.as_ptr(),
            value.raw.numerator.coefficients.as_ptr(),
        );
        assert_ne!(
            copied_numerator.raw.exponents.as_ptr(),
            value.raw.numerator.exponents.as_ptr(),
        );
        assert_ne!(
            copied_denominator.raw.coefficients.as_ptr(),
            value.raw.denominator.coefficients.as_ptr(),
        );
        assert_ne!(
            copied_denominator.raw.exponents.as_ptr(),
            value.raw.denominator.exponents.as_ptr(),
        );
    }

    fn bounded_associate_polynomial(
        context: &ParametricCoefficientContext,
        terms: impl IntoIterator<Item = (Integer, Vec<u16>)>,
    ) -> ParametricPolynomial {
        let terms = terms.into_iter().collect::<Vec<_>>();
        assert!(!terms.is_empty());
        let mut coefficients = Vec::with_capacity(terms.len());
        let mut exponents = Vec::with_capacity(terms.len() * context.variables.len());
        for (coefficient, term_exponents) in terms {
            assert_eq!(term_exponents.len(), context.variables.len());
            assert!(!coefficient.is_zero());
            coefficients.push(coefficient);
            exponents.extend(term_exponents);
        }
        let raw = CoefficientPolynomial::from_coefficient_list(
            coefficients,
            exponents,
            context.variables.clone(),
            &context.template.numerator.ring,
        );
        let polynomial = ParametricPolynomial {
            raw,
            context: context.fingerprint.clone(),
        };
        context
            .validate_polynomial_with_limits(&polynomial, ExactAlgebraLimits::default())
            .unwrap();
        polynomial
    }

    fn bounded_associate_projective_polynomial(
        context: &ParametricCoefficientContext,
        index_supports: &[Vec<u16>],
        group_scales: &[i64],
        base_factor: &[(i64, Vec<u16>)],
    ) -> ParametricPolynomial {
        assert_eq!(index_supports.len(), group_scales.len());
        let base_variables = context.base.variables().len();
        let index_variables = context.index_count();
        let mut terms = Vec::with_capacity(index_supports.len() * base_factor.len());
        for (index_support, &group_scale) in index_supports.iter().zip(group_scales) {
            assert_eq!(index_support.len(), index_variables);
            assert_ne!(group_scale, 0);
            for (base_coefficient, base_exponents) in base_factor {
                assert_eq!(base_exponents.len(), base_variables);
                assert_ne!(*base_coefficient, 0);
                let mut exponents = base_exponents.clone();
                exponents.extend_from_slice(index_support);
                terms.push((
                    Integer::from(group_scale) * Integer::from(*base_coefficient),
                    exponents,
                ));
            }
        }
        bounded_associate_polynomial(context, terms)
    }

    fn bounded_associate_integer_vector_polynomial(
        context: &ParametricCoefficientContext,
        coefficients: Vec<Integer>,
    ) -> ParametricPolynomial {
        assert!(context.base.variables().is_empty());
        assert_eq!(context.index_count(), 1);
        bounded_associate_polynomial(
            context,
            coefficients
                .into_iter()
                .enumerate()
                .map(|(power, coefficient)| (coefficient, vec![u16::try_from(power).unwrap()])),
        )
    }

    fn bounded_associate_resource_fixture(
        scope: &str,
    ) -> (
        ParametricCoefficientContext,
        ParametricPolynomial,
        ParametricPolynomial,
    ) {
        let context =
            ParametricCoefficientContext::try_new(&CoefficientContext::new(["x", "y"]), scope, 2)
                .unwrap();
        let supports = [vec![0, 0], vec![2, 0], vec![0, 3]];
        let scales = [1, -2, 3];
        let left = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &scales,
            &[(1, vec![0, 0]), (2, vec![1, 0]), (-3, vec![0, 2])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &scales,
            &[(5, vec![0, 0]), (-7, vec![2, 0]), (11, vec![1, 1])],
        );
        (context, left, right)
    }

    #[test]
    fn bounded_associate_accepts_projective_base_field_units() {
        let (context, left, right) =
            bounded_associate_resource_fixture("bounded-associate-projective-units");
        let forward = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        let reverse = context
            .polynomial_loci_are_associates_with_census(
                &right,
                &left,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(forward.associated());
        assert!(reverse.associated());
        assert!(forward.stats().cross_terms() > 0);
        assert!(forward.stats().visible_temporary_bytes() > 0);
        assert!(
            forward.stats().visible_temporary_bytes()
                <= forward.stats().visible_temporary_byte_envelope()
        );

        let supports = [vec![0, 0], vec![2, 0], vec![0, 3]];
        let negative_right = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &[1, -2, 3],
            &[(-5, vec![0, 0]), (7, vec![2, 0]), (-11, vec![1, 1])],
        );
        assert!(
            context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &negative_right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
    }

    #[test]
    fn bounded_associate_rejects_nonprojective_coefficient_vectors() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-nonprojective",
            1,
        )
        .unwrap();
        let supports = [vec![0], vec![1], vec![4]];
        let factor = [(1, vec![0]), (2, vec![1])];
        let left =
            bounded_associate_projective_polynomial(&context, &supports, &[1, -2, 3], &factor);
        let right =
            bounded_associate_projective_polynomial(&context, &supports, &[1, -2, 4], &factor);
        assert!(
            !context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
    }

    #[test]
    fn bounded_associate_rejects_mismatched_index_supports() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-support-mismatch",
            2,
        )
        .unwrap();
        let left = bounded_associate_projective_polynomial(
            &context,
            &[vec![0, 0], vec![2, 1]],
            &[1, 3],
            &[(1, vec![0]), (1, vec![1])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &[vec![0, 0], vec![2, 2]],
            &[1, 3],
            &[(2, vec![0]), (-1, vec![2])],
        );
        assert!(
            !context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
    }

    #[test]
    fn bounded_associate_rejects_unequal_index_group_counts_before_pair_work_in_both_orders() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-unequal-group-counts",
            2,
        )
        .unwrap();
        let left = bounded_associate_projective_polynomial(
            &context,
            &[vec![0, 0], vec![2, 1]],
            &[1, -3],
            &[(1, vec![0]), (2, vec![1])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &[vec![0, 0], vec![2, 1], vec![1, 4]],
            &[5, -7, 11],
            &[(3, vec![0]), (-2, vec![2])],
        );

        for (first, second) in [(&left, &right), (&right, &left)] {
            let result = context
                .polynomial_loci_are_associates_with_census(
                    first,
                    second,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap();
            let stats = result.stats();
            assert!(!result.associated());
            assert_eq!(stats.index_groups(), 5);
            assert_eq!(stats.anchor_cost_operations(), 0);
            assert_eq!(stats.cross_terms(), 0);
            assert_eq!(stats.integer_preflight_pairs(), 0);
            assert_eq!(stats.integer_multiplication_bit_work_bound(), 0);
            assert_eq!(stats.integer_accumulation_bit_work_bound(), 0);
            assert!(stats.visible_temporary_bytes() <= stats.visible_temporary_byte_envelope());
        }
    }

    #[test]
    fn bounded_associate_rejects_zero_on_either_side() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-zero",
            1,
        )
        .unwrap();
        let nonzero = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(1), vec![0, 1]),
                (Integer::from(1), vec![1, 0]),
            ],
        );
        let zero = context.numerator_condition(&context.zero()).unwrap();
        for (left, right) in [(&zero, &nonzero), (&nonzero, &zero), (&zero, &zero)] {
            assert!(
                !context
                    .polynomial_loci_are_associates_with_census(
                        left,
                        right,
                        ParametricPolynomialAssociateLimits::default(),
                    )
                    .unwrap()
                    .associated()
            );
        }
    }

    #[test]
    fn bounded_associate_accepts_arbitrary_nonzero_base_only_polynomials() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x", "y"]),
            "bounded-associate-base-only",
            2,
        )
        .unwrap();
        let left = bounded_associate_projective_polynomial(
            &context,
            &[vec![0, 0]],
            &[1],
            &[(1, vec![0, 0]), (-2, vec![3, 0])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &[vec![0, 0]],
            &[1],
            &[(7, vec![0, 1]), (11, vec![2, 2]), (-5, vec![0, 0])],
        );
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(result.associated());
        assert_eq!(result.stats().index_groups(), 2);
        assert_eq!(result.stats().cross_terms(), 0);
    }

    #[test]
    fn bounded_associate_accepts_arbitrary_single_nonzero_index_group() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-single-index-group",
            2,
        )
        .unwrap();
        let support = [vec![5, 9]];
        let left = bounded_associate_projective_polynomial(
            &context,
            &support,
            &[1],
            &[(1, vec![0]), (2, vec![1])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &support,
            &[1],
            &[(3, vec![2]), (-4, vec![4]), (5, vec![0])],
        );
        assert!(
            context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
    }

    #[test]
    fn bounded_associate_handles_multiple_indices_and_noncontiguous_source_groups() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x", "y"]),
            "bounded-associate-interleaved-groups",
            3,
        )
        .unwrap();
        let supports = [vec![0, 0, 0], vec![2, 0, 5], vec![0, 7, 1]];
        let left = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &[1, -3, 5],
            &[(1, vec![0, 0]), (2, vec![1, 0]), (-1, vec![0, 2])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &[1, -3, 5],
            &[(7, vec![0, 0]), (-2, vec![2, 0]), (3, vec![1, 1])],
        );
        let first_index = context.base.variables().len();
        let positions = left
            .raw
            .exponents_iter()
            .enumerate()
            .filter_map(|(term, exponents)| {
                (&exponents[first_index..] == supports[0].as_slice()).then_some(term)
            })
            .collect::<Vec<_>>();
        assert_eq!(positions.len(), 3);
        assert!(positions.windows(2).any(|pair| pair[1] != pair[0] + 1));
        assert!(
            context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
    }

    #[test]
    fn bounded_associate_handles_signed_carries_across_two_pow_64() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "bounded-associate-carry-64",
            1,
        )
        .unwrap();
        let two_pow_64 = Integer::from(1) << 64u32;
        let values = vec![
            two_pow_64.clone() - Integer::from(1),
            -(two_pow_64.clone() + Integer::from(1)),
            two_pow_64.clone() + Integer::from(3),
        ];
        let scale = -(two_pow_64 + Integer::from(5));
        let right_values = values
            .iter()
            .cloned()
            .map(|value| value * scale.clone())
            .collect();
        let left = bounded_associate_integer_vector_polynomial(&context, values);
        let right = bounded_associate_integer_vector_polynomial(&context, right_values);
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(result.associated());
        assert!(result.stats().product_scratch_limbs() >= 4);
    }

    #[test]
    fn bounded_associate_handles_signed_carries_across_two_pow_127() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "bounded-associate-carry-127",
            1,
        )
        .unwrap();
        let two_pow_127 = Integer::from(1) << 127u32;
        let values = vec![
            two_pow_127.clone() - Integer::from(1),
            two_pow_127.clone() + Integer::from(1),
            -((Integer::from(1) << 65u32) + Integer::from(3)),
        ];
        let scale = two_pow_127 + Integer::from(17);
        let right_values = values
            .iter()
            .cloned()
            .map(|value| value * scale.clone())
            .collect();
        let left = bounded_associate_integer_vector_polynomial(&context, values);
        let right = bounded_associate_integer_vector_polynomial(&context, right_values);
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(result.associated());
        assert!(result.stats().product_scratch_limbs() >= 6);
    }

    #[test]
    fn bounded_associate_accumulation_bit_bound_charges_both_signed_passes() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "bounded-associate-accumulation-factor-two",
            1,
        )
        .unwrap();
        let left = bounded_associate_integer_vector_polynomial(
            &context,
            vec![Integer::from(1), Integer::from(1)],
        );
        let right = bounded_associate_integer_vector_polynomial(
            &context,
            vec![Integer::from(1), Integer::from(1)],
        );
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(result.associated());
        assert_eq!(result.stats().cross_terms(), 2);
        // Two products, a three-bit conservative accumulator, and the
        // comparison/update two-pass factor: 2 * 2 * 3.
        assert_eq!(result.stats().integer_accumulation_bit_work_bound(), 12);
    }

    #[test]
    fn bounded_associate_handles_large_gmp_coefficients() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "bounded-associate-large-gmp",
            1,
        )
        .unwrap();
        let values = vec![
            (Integer::from(1) << 4096u32) + Integer::from(19),
            -((Integer::from(1) << 3072u32) + Integer::from(23)),
            (Integer::from(1) << 2048u32) - Integer::from(29),
        ];
        let scale = (Integer::from(1) << 1024u32) + Integer::from(31);
        let right_values = values
            .iter()
            .cloned()
            .map(|value| value * scale.clone())
            .collect();
        let left = bounded_associate_integer_vector_polynomial(&context, values);
        let right = bounded_associate_integer_vector_polynomial(&context, right_values);
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(result.associated());
        assert!(result.stats().product_scratch_limbs() >= 80);
        assert!(result.stats().integer_multiply_limb_operation_bound() > 1_000);
    }

    #[test]
    fn bounded_associate_sums_u16_max_base_exponents_without_polynomial_multiplication() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-u16-max",
            1,
        )
        .unwrap();
        let max = u16::MAX;
        let left = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(1), vec![max, 0]),
                (Integer::from(2), vec![max, 1]),
            ],
        );
        let right = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(3), vec![max, 0]),
                (Integer::from(6), vec![max, 1]),
            ],
        );
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(result.associated());
        assert!(result.stats().base_exponent_addition_bound() > 0);
        assert_eq!(left.raw.degree(0), u16::MAX);
        assert_eq!(right.raw.degree(0), u16::MAX);
    }

    #[test]
    fn bounded_associate_detects_a_one_bit_false_mutation() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "bounded-associate-one-bit-mutation",
            1,
        )
        .unwrap();
        let values = vec![
            (Integer::from(1) << 127u32) + Integer::from(3),
            (Integer::from(1) << 65u32) + Integer::from(5),
            Integer::from(17),
        ];
        let right_values = values
            .iter()
            .cloned()
            .map(|value| value * Integer::from(2))
            .collect();
        let left = bounded_associate_integer_vector_polynomial(&context, values);
        let mut right = bounded_associate_integer_vector_polynomial(&context, right_values);
        assert!(
            context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
        let mutated_term = right
            .raw
            .exponents_iter()
            .position(|exponents| exponents[0] == 1)
            .unwrap();
        right.raw.coefficients[mutated_term] =
            right.raw.coefficients[mutated_term].clone() + Integer::from(1);
        assert!(
            !context
                .polynomial_loci_are_associates_with_census(
                    &left,
                    &right,
                    ParametricPolynomialAssociateLimits::default(),
                )
                .unwrap()
                .associated()
        );
    }

    #[test]
    fn bounded_associate_uses_lexicographically_first_minimum_cost_anchor_tie() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-anchor-tie",
            1,
        )
        .unwrap();
        let left = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(1) << 64u32, vec![0, 0]),
                (Integer::from(1), vec![0, 1]),
                (Integer::from(1), vec![1, 1]),
                (Integer::from(1), vec![0, 2]),
                (Integer::from(1), vec![2, 2]),
            ],
        );
        let right = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(1), vec![0, 0]),
                (Integer::from(1), vec![1, 0]),
                (Integer::from(1), vec![0, 1]),
                (Integer::from(1), vec![0, 2]),
                (Integer::from(1), vec![2, 2]),
            ],
        );
        let result = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(!result.associated());
        assert_eq!(result.stats().anchor_cost_operations(), 15);
        assert_eq!(result.stats().cross_terms(), 11);
        assert_eq!(result.stats().peak_cross_terms(), 6);
        // Groups zero and one tie at cost 11.  Retaining group zero as the
        // anchor pairs its 65-bit coefficient three times: 3*65 + 8 = 203.
        assert_eq!(result.stats().integer_multiplication_bit_work_bound(), 203);
    }

    #[test]
    fn bounded_associate_rejects_a_foreign_authenticated_context() {
        let base = CoefficientContext::new(["x"]);
        let first =
            ParametricCoefficientContext::try_new(&base, "bounded-associate-context-a", 1).unwrap();
        let second =
            ParametricCoefficientContext::try_new(&base, "bounded-associate-context-b", 1).unwrap();
        let left = bounded_associate_polynomial(
            &first,
            [
                (Integer::from(1), vec![0, 0]),
                (Integer::from(1), vec![0, 1]),
            ],
        );
        let foreign = bounded_associate_polynomial(
            &second,
            [
                (Integer::from(1), vec![0, 0]),
                (Integer::from(1), vec![0, 1]),
            ],
        );
        assert_eq!(
            first.polynomial_loci_are_associates_with_census(
                &left,
                &foreign,
                ParametricPolynomialAssociateLimits::default(),
            ),
            Err(ParametricCoefficientError::WrongContext)
        );
    }

    #[test]
    fn polynomial_condition_product_separates_pair_work_from_retained_support() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "polynomial-product-support-bound", 1)
                .unwrap();
        let polynomial = bounded_associate_polynomial(
            &context,
            (0..=3).map(|exponent| (Integer::from(1), vec![exponent])),
        );

        let exact = ExactAlgebraLimits {
            max_exponent: 6,
            max_polynomial_terms: 7,
            max_term_operations: 16,
        };
        let product = context
            .multiply_polynomial_conditions_with_limits(&polynomial, &polynomial, exact)
            .unwrap();
        assert_eq!(product.term_count(), 7);
        assert_eq!(product.raw.degree(0), 6);

        let exponent_one_below = ExactAlgebraLimits {
            max_exponent: 5,
            ..exact
        };
        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits(
                    &polynomial,
                    &polynomial,
                    exponent_one_below,
                )
                .unwrap_err(),
            ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ExponentLimit {
                operation: crate::ExactAlgebraOperation::Multiply,
                variable: 0,
                requested: 6,
                limit: 5,
            })
        );

        let operation_one_below = ExactAlgebraLimits {
            max_term_operations: 15,
            ..exact
        };
        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits(
                    &polynomial,
                    &polynomial,
                    operation_one_below,
                )
                .unwrap_err(),
            ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "exact polynomial multiplication term pairs",
                requested: 16,
                limit: 15,
            })
        );

        let output_one_below = ExactAlgebraLimits {
            max_polynomial_terms: 6,
            ..exact
        };
        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits(
                    &polynomial,
                    &polynomial,
                    output_one_below,
                )
                .unwrap_err(),
            ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "exact polynomial multiplication output terms",
                requested: 7,
                limit: 6,
            })
        );

        // The generic rational path intentionally remains stricter: quotient
        // normalization can densify rational results, so it must not reuse the
        // direct-polynomial output proof.
        let rational = ParametricCoefficient {
            raw: polynomial.raw.clone().into(),
            context: context.fingerprint.clone(),
        };
        assert_eq!(
            context
                .mul_with_limits(&rational, &rational, exact)
                .unwrap_err(),
            ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "exact multiplication numerator terms",
                requested: 16,
                limit: 7,
            })
        );
    }

    #[test]
    fn polynomial_condition_product_clamps_a_large_degree_box_to_pair_count() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "polynomial-product-pair-clamp", 2)
                .unwrap();
        let polynomial = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(1), vec![0, 0]),
                (Integer::from(1), vec![100, 100]),
            ],
        );
        let exact = ExactAlgebraLimits {
            max_exponent: 200,
            max_polynomial_terms: 4,
            max_term_operations: 4,
        };
        let product = context
            .multiply_polynomial_conditions_with_limits(&polynomial, &polynomial, exact)
            .unwrap();
        assert_eq!(product.term_count(), 3);
        assert_eq!(product.raw.degree(0), 200);
        assert_eq!(product.raw.degree(1), 200);

        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits(
                    &polynomial,
                    &polynomial,
                    ExactAlgebraLimits {
                        max_polynomial_terms: 3,
                        ..exact
                    },
                )
                .unwrap_err(),
            ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "exact polynomial multiplication output terms",
                requested: 4,
                limit: 3,
            })
        );
    }

    #[test]
    fn polynomial_condition_native_output_envelope_may_exceed_retained_actual_support() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "polynomial-product-native-envelope",
            1,
        )
        .unwrap();
        let one_plus_x_squared = bounded_associate_polynomial(
            &context,
            [(Integer::from(1), vec![0]), (Integer::from(1), vec![2])],
        );
        let exact = ExactAlgebraLimits {
            max_exponent: 4,
            max_polynomial_terms: 3,
            max_term_operations: 4,
        };

        let product = context
            .multiply_polynomial_conditions_with_limits_and_native_output_bound(
                &one_plus_x_squared,
                &one_plus_x_squared,
                exact,
                4,
            )
            .unwrap();
        assert_eq!(product.term_count(), 3);
        assert_eq!(product.raw.degree(0), 4);
        context
            .validate_polynomial_with_limits(&product, exact)
            .unwrap();
    }

    #[test]
    fn polynomial_condition_native_output_envelope_fails_one_below_before_multiplication() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "polynomial-product-native-envelope-one-below",
            1,
        )
        .unwrap();
        let one_plus_x_squared = bounded_associate_polynomial(
            &context,
            [(Integer::from(1), vec![0]), (Integer::from(1), vec![2])],
        );
        let exact = ExactAlgebraLimits {
            max_exponent: 4,
            max_polynomial_terms: 3,
            max_term_operations: 4,
        };
        let expected = ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
            resource: "exact polynomial multiplication output terms",
            requested: 4,
            limit: 3,
        });

        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits_and_native_output_bound(
                    &one_plus_x_squared,
                    &one_plus_x_squared,
                    exact,
                    3,
                )
                .unwrap_err(),
            expected
        );
        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits(
                    &one_plus_x_squared,
                    &one_plus_x_squared,
                    exact,
                )
                .unwrap_err(),
            expected,
            "the ordinary wrapper must retain its original output-envelope behavior",
        );
    }

    #[test]
    fn polynomial_condition_native_envelope_never_relaxes_actual_retained_term_limit() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(Vec::<String>::new()),
            "polynomial-product-retained-limit",
            1,
        )
        .unwrap();
        let one_plus_x = bounded_associate_polynomial(
            &context,
            [(Integer::from(1), vec![0]), (Integer::from(1), vec![1])],
        );
        let one_plus_x_squared = bounded_associate_polynomial(
            &context,
            [(Integer::from(1), vec![0]), (Integer::from(1), vec![2])],
        );
        let exact = ExactAlgebraLimits {
            max_exponent: 3,
            max_polynomial_terms: 3,
            max_term_operations: 4,
        };

        assert_eq!(
            context
                .multiply_polynomial_conditions_with_limits_and_native_output_bound(
                    &one_plus_x,
                    &one_plus_x_squared,
                    exact,
                    4,
                )
                .unwrap_err(),
            ParametricCoefficientError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                resource: "authenticated polynomial terms",
                requested: 4,
                limit: 3,
            })
        );
    }

    #[test]
    fn bounded_associate_rejects_malformed_sparse_layout_before_pair_work() {
        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new(["x"]),
            "bounded-associate-malformed-layout",
            1,
        )
        .unwrap();
        let valid = bounded_associate_polynomial(
            &context,
            [
                (Integer::from(1), vec![0, 0]),
                (Integer::from(1), vec![0, 1]),
            ],
        );
        let mut malformed = valid.clone();
        malformed.raw.exponents.pop();
        assert!(matches!(
            context.polynomial_loci_are_associates_with_census(
                &valid,
                &malformed,
                ParametricPolynomialAssociateLimits::default(),
            ),
            Err(ParametricCoefficientError::ExactAlgebra(
                ExactAlgebraError::MalformedExponentLayout { .. }
            ))
        ));
    }

    macro_rules! bounded_associate_limit_case {
        ($test_name:ident, $limit_field:ident, $stats_getter:ident, $resource:literal) => {
            #[test]
            fn $test_name() {
                let (context, left, right) =
                    bounded_associate_resource_fixture(stringify!($test_name));
                let baseline = context
                    .polynomial_loci_are_associates_with_census(
                        &left,
                        &right,
                        ParametricPolynomialAssociateLimits::default(),
                    )
                    .unwrap();
                assert!(baseline.associated());
                let requested = baseline.stats().$stats_getter();
                assert!(requested > 0, "{} must be exercised", $resource);

                let mut exact = ParametricPolynomialAssociateLimits::default();
                exact.$limit_field = requested;
                let exact_result = context
                    .polynomial_loci_are_associates_with_census(&left, &right, exact)
                    .unwrap();
                assert!(exact_result.associated());
                assert_eq!(exact_result.stats().$stats_getter(), requested);

                let mut one_below = ParametricPolynomialAssociateLimits::default();
                one_below.$limit_field = requested - 1;
                assert_eq!(
                    context
                        .polynomial_loci_are_associates_with_census(&left, &right, one_below)
                        .unwrap_err(),
                    ParametricCoefficientError::ResourceLimit {
                        resource: $resource,
                        requested,
                        limit: requested - 1,
                    }
                );
            }
        };
    }

    bounded_associate_limit_case!(
        bounded_associate_limit_context_fingerprint_bytes,
        max_context_fingerprint_comparison_bytes,
        context_fingerprint_comparison_bytes,
        "polynomial-associate context fingerprint comparison bytes"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_variable_map_comparisons,
        max_variable_map_entry_comparisons,
        variable_map_entry_comparisons,
        "polynomial-associate variable-map entry comparisons"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_validation_terms,
        max_validation_terms,
        validation_terms,
        "polynomial-associate validation terms"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_validation_exponent_entries,
        max_validation_exponent_entries,
        validation_exponent_entries,
        "polynomial-associate validation exponent entries"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_validation_integer_bits,
        max_validation_integer_bits,
        validation_integer_bits,
        "polynomial-associate validation integer bits"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_magnitude_copy_terms,
        max_magnitude_copy_terms,
        magnitude_copy_terms,
        "polynomial-associate magnitude copy terms"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_magnitude_copy_bytes,
        max_magnitude_copy_bytes,
        magnitude_copy_bytes,
        "polynomial-associate magnitude copy bytes"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_term_order_entries,
        max_term_order_entries,
        term_order_entries,
        "polynomial-associate term-order entries"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_index_groups,
        max_index_groups,
        index_groups,
        "polynomial-associate index groups"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_ordering_comparisons,
        max_ordering_comparison_bound,
        ordering_comparison_bound,
        "polynomial-associate ordering comparison bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_index_coordinate_inspections,
        max_index_coordinate_inspection_bound,
        index_coordinate_inspection_bound,
        "polynomial-associate index-coordinate inspection bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_base_sum_coordinate_inspections,
        max_base_sum_coordinate_inspection_bound,
        base_sum_coordinate_inspection_bound,
        "polynomial-associate base-sum coordinate inspection bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_base_exponent_additions,
        max_base_exponent_addition_bound,
        base_exponent_addition_bound,
        "polynomial-associate base-exponent addition bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_anchor_cost_operations,
        max_anchor_cost_operations,
        anchor_cost_operations,
        "polynomial-associate anchor cost operations"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_cross_terms,
        max_cross_terms,
        cross_terms,
        "polynomial-associate cross terms"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_peak_cross_terms,
        max_peak_cross_terms,
        peak_cross_terms,
        "polynomial-associate peak cross terms"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_integer_preflight_pairs,
        max_integer_preflight_pairs,
        integer_preflight_pairs,
        "polynomial-associate integer preflight pairs"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_integer_multiplication_bit_work,
        max_integer_multiplication_bit_work_bound,
        integer_multiplication_bit_work_bound,
        "polynomial-associate integer multiplication bit-work bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_integer_accumulation_bit_work,
        max_integer_accumulation_bit_work_bound,
        integer_accumulation_bit_work_bound,
        "polynomial-associate integer accumulation bit-work bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_integer_multiply_limb_operations,
        max_integer_multiply_limb_operation_bound,
        integer_multiply_limb_operation_bound,
        "polynomial-associate integer multiply limb-operation bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_integer_accumulate_limb_operations,
        max_integer_accumulate_limb_operation_bound,
        integer_accumulate_limb_operation_bound,
        "polynomial-associate integer accumulation limb-operation bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_scratch_limb_writes,
        max_scratch_limb_write_bound,
        scratch_limb_write_bound,
        "polynomial-associate scratch limb-write bound"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_product_scratch_limbs,
        max_product_scratch_limbs,
        product_scratch_limbs,
        "polynomial-associate product scratch limbs"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_accumulator_scratch_limbs,
        max_accumulator_scratch_limbs,
        accumulator_scratch_limbs,
        "polynomial-associate accumulator scratch limbs"
    );
    bounded_associate_limit_case!(
        bounded_associate_limit_visible_temporary_byte_envelope,
        max_visible_temporary_byte_envelope,
        visible_temporary_byte_envelope,
        "polynomial-associate visible temporary byte envelope"
    );

    #[test]
    fn bounded_associate_resource_limit_error_does_not_expose_private_payloads() {
        const CONTEXT_SENTINEL: &str = "privacy-context-payload-sentinel-746381";
        const POLYNOMIAL_SENTINEL: &str = "polyprivacyuniquesentinel746381";

        let context = ParametricCoefficientContext::try_new(
            &CoefficientContext::new([POLYNOMIAL_SENTINEL]),
            CONTEXT_SENTINEL,
            2,
        )
        .unwrap();
        let supports = [vec![0, 0], vec![2, 0], vec![0, 3]];
        let left = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &[1, -2, 3],
            &[(1, vec![0]), (2, vec![1]), (-3, vec![2])],
        );
        let right = bounded_associate_projective_polynomial(
            &context,
            &supports,
            &[1, -2, 3],
            &[(5, vec![0]), (-7, vec![2]), (11, vec![1])],
        );
        let baseline = context
            .polynomial_loci_are_associates_with_census(
                &left,
                &right,
                ParametricPolynomialAssociateLimits::default(),
            )
            .unwrap();
        assert!(baseline.associated());
        let requested = baseline.stats().cross_terms();
        assert!(requested > 0);

        let mut one_below = ParametricPolynomialAssociateLimits::default();
        one_below.max_cross_terms = requested - 1;
        let error = context
            .polynomial_loci_are_associates_with_census(&left, &right, one_below)
            .unwrap_err();
        assert_eq!(
            error,
            ParametricCoefficientError::ResourceLimit {
                resource: "polynomial-associate cross terms",
                requested,
                limit: requested - 1,
            }
        );

        for rendering in [format!("{error:?}"), error.to_string()] {
            assert!(!rendering.contains(CONTEXT_SENTINEL));
            assert!(!rendering.contains(POLYNOMIAL_SENTINEL));
        }
    }

    #[test]
    fn polynomial_associate_proof_accepts_only_base_field_units() {
        let base = CoefficientContext::new(["theta"]);
        let context = ParametricCoefficientContext::try_new(&base, "associates", 1).unwrap();
        let n = context.index(0).unwrap();
        let p = context.add(&n, &context.one()).unwrap();
        let p = context.numerator_condition(&p).unwrap();
        let minus_p = context
            .numerator_condition(
                &context
                    .neg(&context.add(&n, &context.one()).unwrap())
                    .unwrap(),
            )
            .unwrap();
        assert!(
            context
                .polynomial_loci_are_associates_with_limits(
                    &p,
                    &minus_p,
                    ExactAlgebraLimits::default(),
                )
                .unwrap()
        );

        let theta = context.lift(&base.parameter("theta").unwrap()).unwrap();
        let theta_p = context
            .mul(&theta, &context.add(&n, &context.one()).unwrap())
            .unwrap();
        let theta_p = context.numerator_condition(&theta_p).unwrap();
        assert!(
            context
                .polynomial_loci_are_associates_with_limits(
                    &p,
                    &theta_p,
                    ExactAlgebraLimits::default(),
                )
                .unwrap()
        );

        let q = context.add(&n, &context.integer(2)).unwrap();
        let q = context.numerator_condition(&q).unwrap();
        assert!(
            !context
                .polynomial_loci_are_associates_with_limits(&p, &q, ExactAlgebraLimits::default(),)
                .unwrap()
        );
        let zero = context.numerator_condition(&context.zero()).unwrap();
        assert!(
            !context
                .polynomial_loci_are_associates_with_limits(
                    &zero,
                    &p,
                    ExactAlgebraLimits::default(),
                )
                .unwrap()
        );
    }

    #[test]
    fn specialized_nonzero_condition_rejects_empty_provenance() {
        let base = CoefficientContext::new(["x"]);
        let polynomial = BasePolynomial::try_from_raw(
            base.parameter("x").unwrap().numerator.clone(),
            &base,
            ExactAlgebraLimits::default(),
        )
        .unwrap();
        assert!(matches!(
            SpecializedNonZeroCondition::from_base_polynomial(
                polynomial,
                Vec::<GuardOrigin>::new(),
                1,
            ),
            Err(ParametricCoefficientError::MissingGuardOrigin)
        ));
    }

    #[test]
    fn generated_affine_guard_seal_preserves_polynomial_and_erases_private_vectors() {
        let base = CoefficientContext::new(["x"]);
        let polynomial = BasePolynomial::try_from_raw(
            base.parse("x+1").unwrap().numerator,
            &base,
            ExactAlgebraLimits::default(),
        )
        .unwrap();
        let expected_polynomial = polynomial.clone();
        let mut condition = SpecializedNonZeroCondition::from_base_polynomial(
            polynomial,
            [
                GuardOrigin::RelationAffineFreeRecentering {
                    source_row: GuardRowId::Derived {
                        label: Arc::from("private-source"),
                    },
                    target_row: GuardRowId::Derived {
                        label: Arc::from("private-target"),
                    },
                    coefficient_offset: vec![7, -11],
                    key_center: vec![13, -17],
                },
                GuardOrigin::IndexSpecialization {
                    assignment: vec![19, -23].into_boxed_slice(),
                },
                GuardOrigin::RelationResidualAffineBranchSubstitutionTermDenominator {
                    row: GuardRowId::Derived {
                        label: Arc::from("private-row"),
                    },
                    shift: vec![29, -31].into_boxed_slice(),
                    source_case: 37,
                    source_work_item_ordinal: 41,
                    ready_terminal_ordinal: 43,
                },
            ],
            3,
        )
        .unwrap();

        condition.seal_generated_affine_provenance();

        assert_eq!(condition.polynomial(), &expected_polynomial);
        assert_eq!(
            condition.origins(),
            &BTreeSet::from([GuardOrigin::GeneratedAffineSealedCondition])
        );
        assert_eq!(
            condition.origins().iter().next().unwrap().stable_string(),
            "generated-affine-sealed-condition"
        );

        // Re-sealing is deterministic and does not perturb either field.
        condition.seal_generated_affine_provenance();
        assert_eq!(condition.polynomial(), &expected_polynomial);
        assert_eq!(
            condition.origins(),
            &BTreeSet::from([GuardOrigin::GeneratedAffineSealedCondition])
        );
    }

    #[test]
    fn translation_preflight_covers_canonical_automorphic_fraction() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "translation-preflight-canonical", 1)
                .unwrap();
        let n = context.index(0).unwrap();
        let n2 = context.mul(&n, &n).unwrap();
        let n4 = context.mul(&n2, &n2).unwrap();
        let numerator = context.sub(&n4, &context.one()).unwrap();
        let denominator = context.add(&n, &context.integer(2)).unwrap();
        let value = context.checked_div(&numerator, &denominator).unwrap();
        assert_eq!(value.raw.numerator.nterms(), 2);
        assert_eq!(value.raw.denominator.nterms(), 2);

        let shift = IndexShift::try_new([1], 1).unwrap();
        let preflight = context
            .preflight_translate_coefficient(&value, &shift, ParametricArithmeticLimits::default())
            .unwrap();
        assert_eq!(preflight.source_terms(), 4);
        assert_eq!(preflight.output_term_bound(), 9);
        assert_eq!(preflight.normalization_input_term_pair_bound(), 0);
        assert_eq!(preflight.power_operation_bound(), 2);

        let translated = context
            .translate(
                &value,
                shift.values(),
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        // Translation preserves the canonical fraction's coprimality, while
        // sparse expansion and cancellation still change its support.
        assert_eq!(translated.raw.numerator.nterms(), 4);
        assert_eq!(translated.raw.denominator.nterms(), 2);
        assert!(
            translated.raw.numerator.nterms() + translated.raw.denominator.nterms()
                <= preflight.normalized_coefficient_term_bound()
        );
        assert!(
            translated.owned_retained_byte_bound().unwrap()
                <= preflight.normalized_coefficient_byte_bound()
        );
    }

    #[test]
    fn exact_translation_canonicalizes_integer_variants_and_matches_i64_path() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "exact-translation-canonical", 2).unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let numerator = context
            .add(&context.mul(&n0, &n1).unwrap(), &context.integer(5))
            .unwrap();
        let denominator = context
            .add(&context.sub(&n0, &n1).unwrap(), &context.integer(7))
            .unwrap();
        let coefficient = context.checked_div(&numerator, &denominator).unwrap();
        let polynomial = context.numerator_condition(&numerator).unwrap();

        let noncanonical = [
            Integer::Double(2),
            Integer::Large(MultiPrecisionInteger::from(-3)),
        ];
        let compact = [2, -3];
        assert_eq!(
            context
                .preflight_translate_coefficient_exact(
                    &coefficient,
                    &noncanonical,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap(),
            context
                .preflight_translate_coefficient(
                    &coefficient,
                    &IndexShift::try_new(compact, 2).unwrap(),
                    ParametricArithmeticLimits::default(),
                )
                .unwrap()
        );
        assert_eq!(
            context
                .translate_exact(
                    &coefficient,
                    &noncanonical,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap(),
            context
                .translate(
                    &coefficient,
                    &compact,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap()
        );
        assert_eq!(
            context
                .translate_polynomial_exact(
                    &polynomial,
                    &noncanonical,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap(),
            context
                .translate_polynomial(&polynomial, &compact, ParametricArithmeticLimits::default(),)
                .unwrap()
        );

        for exact in [
            [Integer::Double(0), Integer::Large(0.into())],
            [Integer::Large(1.into()), Integer::Double(-1)],
        ] {
            let compact = if exact[0].cmp(&Integer::Single(0)) == Ordering::Equal {
                [0, 0]
            } else {
                [1, -1]
            };
            assert_eq!(
                context
                    .translate_exact(&coefficient, &exact, ParametricArithmeticLimits::default(),)
                    .unwrap(),
                context
                    .translate(
                        &coefficient,
                        &compact,
                        ParametricArithmeticLimits::default(),
                    )
                    .unwrap()
            );
        }

        let minimum = [Integer::Double(i128::from(i64::MIN)), Integer::Double(0)];
        assert_eq!(
            context
                .translate_polynomial_exact(
                    &polynomial,
                    &minimum,
                    ParametricArithmeticLimits::default(),
                )
                .unwrap(),
            context
                .translate_polynomial(
                    &polynomial,
                    &[i64::MIN, 0],
                    ParametricArithmeticLimits::default(),
                )
                .unwrap()
        );
        assert!(matches!(
            context.translate_exact(
                &coefficient,
                &[Integer::from(1)],
                ParametricArithmeticLimits::default(),
            ),
            Err(ParametricCoefficientError::WrongIndexArity {
                expected: 2,
                actual: 1,
            })
        ));

        let oversized_large = || {
            let mut value = MultiPrecisionInteger::with_capacity(1_000_000);
            value += 1;
            value <<= 200_u32;
            value += 37;
            assert!(value.capacity() >= 1_000_000);
            value
        };
        let used = oversized_large();
        let input_capacity = used.capacity();
        let used_shift = [Integer::Large(used), Integer::from(0)];
        let used_preflight = context
            .preflight_translate_polynomial_exact(
                &polynomial,
                &used_shift,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let used_output = context
            .translate_polynomial_exact(
                &polynomial,
                &used_shift,
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert!(
            used_output.owned_retained_byte_bound().unwrap()
                <= used_preflight.retained_output_byte_bound()
        );
        assert!(
            used_output
                .raw
                .coefficients
                .iter()
                .all(|value| match value {
                    Integer::Large(value) => value.capacity() < input_capacity,
                    Integer::Single(_) | Integer::Double(_) => true,
                })
        );

        // A huge, spare-capacity offset on an absent variable performs no GMP
        // cloning or substitution work, so no extra offset bits are charged
        // under the one-bit unit-coefficient ceiling.
        let irrelevant = oversized_large();
        let only_n0 = context.numerator_condition(&n0).unwrap();
        let strict = ParametricArithmeticLimits {
            max_specialization_integer_bits: 1,
            ..ParametricArithmeticLimits::default()
        };
        assert_eq!(
            context
                .translate_polynomial_exact(
                    &only_n0,
                    &[Integer::from(0), Integer::Large(irrelevant)],
                    strict,
                )
                .unwrap(),
            only_n0
        );
    }

    #[test]
    fn exact_gmp_translation_matches_symbolica_shift_and_evaluation_oracles() {
        let base = CoefficientContext::new(["d", "m2"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "exact-gmp-translation-differential", 2)
                .unwrap();
        let d = context.lift(&base.parameter("d").unwrap()).unwrap();
        let m2 = context.lift(&base.parameter("m2").unwrap()).unwrap();
        let n0 = context.index(0).unwrap();
        let n1 = context.index(1).unwrap();
        let n0_squared = context.mul(&n0, &n0).unwrap();
        let n0_cubed = context.mul(&n0_squared, &n0).unwrap();
        let n1_squared = context.mul(&n1, &n1).unwrap();
        let high_degree = context.mul(&n0_cubed, &n1_squared).unwrap();
        let first = context
            .mul(&context.add(&d, &context.integer(2)).unwrap(), &high_degree)
            .unwrap();
        let mixed = context.mul(&n0, &n1).unwrap();
        let second = context
            .mul(&context.sub(&m2, &context.integer(7)).unwrap(), &mixed)
            .unwrap();
        let numerator = context
            .add(&context.add(&first, &second).unwrap(), &context.integer(11))
            .unwrap();
        let denominator = context
            .add(&context.add(&n0, &n1).unwrap(), &context.integer(1))
            .unwrap();
        let coefficient = context.checked_div(&numerator, &denominator).unwrap();
        let polynomial = context.numerator_condition(&numerator).unwrap();

        let mut positive = MultiPrecisionInteger::from(1);
        positive <<= 300_u32;
        positive += 17;
        let positive = Integer::from(positive);
        let mut negative_magnitude = MultiPrecisionInteger::from(1);
        negative_magnitude <<= 333_u32;
        negative_magnitude += 19;
        let negative = -Integer::from(negative_magnitude);
        let shift = [positive.clone(), negative.clone()];
        assert_eq!(integer_magnitude_bits(&shift[0]), 301);
        assert_eq!(integer_magnitude_bits(&shift[1]), 334);

        let limits = ParametricArithmeticLimits::default();
        let preflight = context
            .preflight_translate_polynomial_exact(&polynomial, &shift, limits)
            .unwrap();
        let translated = context
            .translate_polynomial_exact(&polynomial, &shift, limits)
            .unwrap();
        assert!(translated.raw.nterms() <= preflight.output_term_bound());
        assert!(translated.raw.exponents.len() <= preflight.output_exponent_entry_bound());
        assert!(
            translated
                .raw
                .coefficients
                .iter()
                .all(|value| integer_magnitude_bits(value)
                    <= preflight.largest_output_integer_bit_bound() as u128)
        );
        assert!(
            translated.owned_retained_byte_bound().unwrap()
                <= preflight.retained_output_byte_bound()
        );

        // Independent native Symbolica implementation of x -> x+a.
        let base_count = context.base.variables().len();
        let shifted_oracle = polynomial
            .raw
            .shift_var(base_count, &positive)
            .shift_var(base_count + 1, &negative);
        assert_eq!(translated.raw, shifted_oracle);
        assert_eq!(
            translated.raw,
            polynomial
                .raw
                .shift_var(base_count + 1, &negative)
                .shift_var(base_count, &positive)
        );
        let translated_coefficient = context
            .translate_exact(&coefficient, &shift, limits)
            .unwrap();
        assert_eq!(
            translated_coefficient.raw.numerator,
            coefficient
                .raw
                .numerator
                .shift_var(base_count, &positive)
                .shift_var(base_count + 1, &negative)
        );
        assert_eq!(
            translated_coefficient.raw.denominator,
            coefficient
                .raw
                .denominator
                .shift_var(base_count, &positive)
                .shift_var(base_count + 1, &negative)
        );

        // Degree-complete exact point evaluation provides a second oracle.
        for d_value in [0, 3] {
            for m2_value in [-2, 5] {
                for n0_value in 0..=3 {
                    for n1_value in 0..=2 {
                        let target_point = [
                            Integer::from(d_value),
                            Integer::from(m2_value),
                            Integer::from(n0_value),
                            Integer::from(n1_value),
                        ];
                        let source_point = [
                            Integer::from(d_value),
                            Integer::from(m2_value),
                            &Integer::from(n0_value) + &positive,
                            &Integer::from(n1_value) + &negative,
                        ];
                        assert_eq!(target_point.len(), translated.raw.nvars());
                        assert_eq!(source_point.len(), polynomial.raw.nvars());
                        assert_eq!(
                            translated.raw.replace_all(&target_point),
                            polynomial.raw.replace_all(&source_point)
                        );
                    }
                }
            }
        }

        let inverse = [-positive.clone(), -negative.clone()];
        assert_eq!(
            context
                .translate_polynomial_exact(&translated, &inverse, limits)
                .unwrap(),
            polynomial
        );
        assert_eq!(
            context
                .translate_exact(&translated_coefficient, &inverse, limits)
                .unwrap(),
            coefficient
        );
        let followup = [Integer::from(7), Integer::from(-11)];
        let composed = context
            .translate_polynomial_exact(
                &context
                    .translate_polynomial_exact(&polynomial, &shift, limits)
                    .unwrap(),
                &followup,
                limits,
            )
            .unwrap();
        let summed = [&positive + &followup[0], &negative + &followup[1]];
        assert_eq!(
            composed,
            context
                .translate_polynomial_exact(&polynomial, &summed, limits)
                .unwrap()
        );

        let cancellation = context
            .numerator_condition(&context.add(&n0, &n1).unwrap())
            .unwrap();
        assert_eq!(
            context
                .translate_polynomial_exact(
                    &cancellation,
                    &[positive.clone(), -positive.clone()],
                    limits,
                )
                .unwrap(),
            cancellation
        );

        for strict in [
            ParametricArithmeticLimits {
                max_output_terms: preflight.output_term_bound() - 1,
                ..limits
            },
            ParametricArithmeticLimits {
                max_specialization_power_operations: preflight.power_operation_bound() - 1,
                ..limits
            },
            ParametricArithmeticLimits {
                max_specialization_integer_bits: preflight.largest_output_integer_bit_bound() - 1,
                ..limits
            },
        ] {
            assert!(matches!(
                context.preflight_translate_polynomial_exact(&polynomial, &shift, strict),
                Err(ParametricCoefficientError::ResourceLimit { .. })
            ));
        }
    }

    #[test]
    fn specialization_preflight_covers_normalized_value_and_denominator_guard() {
        let base = CoefficientContext::new(["x"]);
        let context =
            ParametricCoefficientContext::try_new(&base, "specialization-preflight", 1).unwrap();
        let x = context.lift(&base.parameter("x").unwrap()).unwrap();
        let n = context.index(0).unwrap();
        let numerator = context.add(&x, &n).unwrap();
        let denominator = context
            .sub(&context.mul(&x, &n).unwrap(), &context.one())
            .unwrap();
        let fabricated = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: numerator.raw.numerator.clone(),
                denominator: denominator.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let preflight = context
            .preflight_specialize_coefficient(
                &fabricated,
                &[2],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        assert_eq!(preflight.source_terms(), 4);
        assert_eq!(preflight.output_term_bound(), 4);
        assert_eq!(preflight.power_operation_bound(), 4);
        assert_eq!(preflight.normalization_input_term_pair_bound(), 4);
        assert_eq!(preflight.denominator_guard_term_bound(), 2);

        let specialized = context
            .specialize(&fabricated, &[2], ParametricArithmeticLimits::default())
            .unwrap();
        assert_eq!(specialized.guarded_nonzero_conditions().len(), 1);
        let guard = &specialized.guarded_nonzero_conditions()[0];
        assert!(guard.polynomial().raw().nterms() <= preflight.denominator_guard_term_bound());
        assert!(
            guard.polynomial().owned_retained_byte_bound().unwrap()
                <= preflight.denominator_guard_byte_bound()
        );
        assert!(
            specialized.value.numerator.nterms() + specialized.value.denominator.nterms()
                <= preflight.normalized_coefficient_term_bound()
        );
    }

    #[test]
    fn shared_preflights_reject_one_below_native_work_before_execution() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "preflight-one-below", 1).unwrap();
        let n = context.index(0).unwrap();
        let n2 = context.mul(&n, &n).unwrap();
        let polynomial = context.numerator_condition(&n2).unwrap();
        let shift = IndexShift::try_new([1], 1).unwrap();
        let no_powers = ParametricArithmeticLimits {
            max_specialization_power_operations: 0,
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            context.preflight_translate_polynomial(&polynomial, &shift, no_powers),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "parametric translation power operations",
                requested: 1,
                limit: 0,
            })
        ));

        let two_term_polynomial = context
            .numerator_condition(&context.add(&n, &context.one()).unwrap())
            .unwrap();
        let one_output_term = ParametricArithmeticLimits {
            max_output_terms: 1,
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            context.preflight_specialize_polynomial(&two_term_polynomial, &[2], one_output_term,),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "coefficient specialization output terms",
                requested: 2,
                limit: 1,
            })
        ));

        let numerator = context.add(&n, &context.one()).unwrap();
        let denominator = context.sub(&n, &context.one()).unwrap();
        let fabricated = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: numerator.raw.numerator.clone(),
                denominator: denominator.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let below_normalization = ParametricArithmeticLimits {
            exact_algebra: ExactAlgebraLimits {
                max_term_operations: 3,
                ..ExactAlgebraLimits::default()
            },
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            context.specialize(&fabricated, &[2], below_normalization),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "coefficient specialization normalization input term pairs",
                requested: 4,
                limit: 3,
            })
        ));
    }

    #[test]
    fn translation_guard_origin_limit_precedes_polynomial_translation() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "translation-origin-preflight", 1)
                .unwrap();
        let polynomial = context
            .numerator_condition(&context.index(0).unwrap())
            .unwrap();
        let condition = context
            .nonzero_condition_with_origins_and_limits(
                polynomial,
                [GuardOrigin::GuardedDivisionDivisorNumerator],
                ExactAlgebraLimits::default(),
            )
            .unwrap();
        let limits = ParametricArithmeticLimits {
            max_source_terms: 0,
            max_guard_origins: 1,
            ..ParametricArithmeticLimits::default()
        };
        assert!(matches!(
            context.translate_nonzero_condition(&condition, &[1], limits),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "parametric guard origins",
                requested: 2,
                limit: 1,
            })
        ));
    }

    #[test]
    fn lift_translate_and_specialize_preserve_authenticated_maps() {
        let base = CoefficientContext::new(["d", "m2"]);
        let context = ParametricCoefficientContext::try_new(&base, "translation", 2).unwrap();
        let d = base.parameter("d").unwrap();
        let m2 = base.parameter("m2").unwrap();
        let family_value = &(&d + &base.integer(1)) / &m2;
        let lifted = context.lift(&family_value).unwrap();
        let n0 = context.index(0).unwrap();
        let value = context.mul(&n0, &lifted).unwrap();
        let translated = context
            .translate(&value, &[2, -3], ParametricArithmeticLimits::default())
            .unwrap();
        let specialized = context
            .specialize(
                &translated,
                &[5, 100],
                ParametricArithmeticLimits::default(),
            )
            .unwrap();
        let expected = &base.integer(7) * &family_value;
        assert_eq!(specialized.value, expected);
        assert_eq!(specialized.nonzero.len(), 1);
        assert_eq!(specialized.nonzero[0].to_expression(), m2.to_expression());
    }

    #[test]
    fn specialization_retains_a_cancelled_index_dependent_pole() {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "cancelled-pole", 1).unwrap();
        let n = context.index(0).unwrap();
        let one = context.one();
        let n_minus_one = context.sub(&n, &one).unwrap();
        let fabricated = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: n_minus_one.raw.numerator.clone(),
                denominator: n_minus_one.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };
        let generic = context
            .specialize(&fabricated, &[2], ParametricArithmeticLimits::default())
            .unwrap();
        assert_eq!(generic.value, base.one());
        assert!(
            generic.nonzero.is_empty(),
            "constant nonzero guards are tautologies"
        );
        assert!(matches!(
            context.specialize(&fabricated, &[1], ParametricArithmeticLimits::default(),),
            Err(ParametricCoefficientError::ZeroDenominator)
        ));
    }

    #[test]
    fn rejects_foreign_maps_before_symbolica_can_unify_them() {
        let base = CoefficientContext::new(["d"]);
        let foreign = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "strict-map", 1).unwrap();
        assert!(matches!(
            context.lift(&foreign.one()),
            Err(ParametricCoefficientError::WrongContext)
        ));
        assert!(matches!(
            context.translate(&context.one(), &[], ParametricArithmeticLimits::default()),
            Err(ParametricCoefficientError::WrongIndexArity { .. })
        ));
    }

    #[test]
    fn parametric_authentication_rejects_malformed_layout_before_arithmetic() {
        let base = CoefficientContext::new(["x"]);
        let context = ParametricCoefficientContext::try_new(&base, "malformed", 1).unwrap();
        let mut malformed = context.one();
        malformed.raw.numerator.exponents.push(0);

        assert!(!context.contains(&malformed));
        assert!(matches!(
            context.add(&malformed, &context.one()),
            Err(ParametricCoefficientError::ExactAlgebra(
                ExactAlgebraError::MalformedExponentLayout { .. }
            ))
        ));
    }

    #[test]
    fn guarded_division_retains_divisor_numerator_for_n_over_n_and_zero_over_n() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "guarded-division", 1).unwrap();
        let n = context.index(0).unwrap();

        let n_over_n = context.checked_div_guarded(&n, &n).unwrap();
        assert_eq!(n_over_n.value, context.one());
        assert_eq!(n_over_n.nonzero.len(), 1);
        assert_eq!(
            n_over_n.nonzero[0].polynomial(),
            &context.numerator_condition(&n).unwrap()
        );
        assert_eq!(
            n_over_n.nonzero[0].origins(),
            &BTreeSet::from([GuardOrigin::GuardedDivisionDivisorNumerator])
        );

        let zero_over_n = context.checked_div_guarded(&context.zero(), &n).unwrap();
        assert!(zero_over_n.value.is_zero());
        assert_eq!(zero_over_n.nonzero, n_over_n.nonzero);
    }

    #[test]
    fn guarded_division_merges_duplicate_polynomial_origins_before_cancellation() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "division-origin-merge", 1).unwrap();
        let n = context.index(0).unwrap();
        let deliberately_uncancelled = ParametricCoefficient {
            raw: RationalPolynomial {
                numerator: n.raw.numerator.clone(),
                denominator: n.raw.numerator.clone(),
            },
            context: context.fingerprint.clone(),
        };

        let divided = context
            .checked_div_guarded(&context.one(), &deliberately_uncancelled)
            .unwrap();
        assert_eq!(divided.value, context.one());
        assert_eq!(divided.nonzero.len(), 1);
        assert_eq!(
            divided.nonzero[0].origins(),
            &BTreeSet::from([
                GuardOrigin::GuardedDivisionDivisorDenominator,
                GuardOrigin::GuardedDivisionDivisorNumerator,
            ])
        );
    }

    #[test]
    fn guarded_division_obeys_caller_exact_limits() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "guarded-division-limits", 1).unwrap();
        let n = context.index(0).unwrap();
        let strict = ExactAlgebraLimits {
            max_exponent: 0,
            ..ExactAlgebraLimits::default()
        };
        assert!(matches!(
            context.checked_div_guarded_with_limits(&context.zero(), &n, strict),
            Err(ParametricCoefficientError::ExactAlgebra(
                ExactAlgebraError::ExponentLimit {
                    operation: crate::ExactAlgebraOperation::Authenticate,
                    requested: 1,
                    limit: 0,
                    ..
                }
            ))
        ));
    }
}
