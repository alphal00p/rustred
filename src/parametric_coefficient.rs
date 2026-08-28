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
use std::mem::size_of;
use std::sync::Arc;

use symbolica::atom::{NamespacedSymbol, SymbolBuilder};
use symbolica::domains::rational_polynomial::FromNumeratorAndDenominator;
use symbolica::prelude::*;

use crate::GuardOrigin;
use crate::algebra::{
    ExactAlgebraError, ExactAlgebraLimits, checked_coefficient_add_on_map,
    checked_coefficient_div_on_map, checked_coefficient_mul_on_map, checked_coefficient_neg_on_map,
    checked_coefficient_sub_on_map, checked_polynomial_mul_on_map, validate_coefficient_on_map,
    validate_polynomial_on_map,
};
use crate::{IndexShift, algebra::Coefficient, algebra::CoefficientContext};

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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
                crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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

    pub fn contains_base_polynomial(&self, value: &BasePolynomial) -> bool {
        value.context.as_ref() == self.base_fingerprint.as_ref()
            && validate_polynomial_on_map(
                &value.raw,
                self.base.variables(),
                crate::algebra::CoefficientPolynomialPart::Numerator,
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

    /// Lift one arbitrary-precision Symbolica integer into `K(n)` without an
    /// intermediate machine-integer conversion.
    ///
    /// The magnitude is admitted before the first GMP-backed copy.  Actual
    /// construction is delegated to Symbolica's public polynomial-constant
    /// API, and the result crosses the same checked variable-map boundary as
    /// every other parametric coefficient.  This crate-private seam is used
    /// by exact affine-domain compilers when a boundary value does not fit in
    /// `i64`.
    pub(crate) fn integer_exact(
        &self,
        value: &Integer,
        limits: ParametricArithmeticLimits,
    ) -> Result<ParametricCoefficient, ParametricCoefficientError> {
        let requested = usize::try_from(value.magnitude_bits()).map_err(|_| {
            ParametricCoefficientError::ResourceCountOverflow {
                resource: "exact integer constant bits",
            }
        })?;
        check_limit(
            "exact integer constant bits",
            requested,
            limits.max_specialization_integer_bits,
        )?;
        let raw = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.template
                .numerator
                .constant(value.to_canonical_integer())
                .into()
        }))
        .map_err(|_| {
            ParametricCoefficientError::Symbolica(
                "Symbolica panicked while constructing an exact integer constant".to_owned(),
            )
        })?;
        self.wrap_checked_with_limits(raw, limits.exact_algebra)
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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
            crate::algebra::CoefficientPolynomialPart::Numerator,
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

    #[test]
    fn base_field_may_be_q_and_indices_remain_distinct() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context = ParametricCoefficientContext::try_new(&base, "empty-base", 2).unwrap();
        assert_eq!(base.parameter_names(), &[] as &[String]);
        assert_eq!(context.index_count(), 2);
        assert!(context.contains(&context.index(0).unwrap()));
    }

    fn polynomial_from_terms_for_test(
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

    #[test]
    fn polynomial_condition_product_separates_pair_work_from_retained_support() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "polynomial-product-support-bound", 1)
                .unwrap();
        let polynomial = polynomial_from_terms_for_test(
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
                operation: crate::algebra::ExactAlgebraOperation::Multiply,
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
        let polynomial = polynomial_from_terms_for_test(
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
        let one_plus_x_squared = polynomial_from_terms_for_test(
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
        let one_plus_x_squared = polynomial_from_terms_for_test(
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
        let one_plus_x = polynomial_from_terms_for_test(
            &context,
            [(Integer::from(1), vec![0]), (Integer::from(1), vec![1])],
        );
        let one_plus_x_squared = polynomial_from_terms_for_test(
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
    fn exact_integer_constant_uses_symbolica_gmp_without_i64_narrowing() {
        let base = CoefficientContext::new(Vec::<String>::new());
        let context =
            ParametricCoefficientContext::try_new(&base, "exact-integer-constant", 1).unwrap();

        for noncanonical_zero in [
            Integer::Double(0),
            Integer::Large(MultiPrecisionInteger::from(0)),
        ] {
            assert_eq!(
                context
                    .integer_exact(&noncanonical_zero, ParametricArithmeticLimits::default(),)
                    .unwrap(),
                context.zero(),
                "every public Symbolica zero representation must cross as canonical K(n) zero",
            );
        }

        for exact in [
            Integer::Double(7),
            Integer::Large(MultiPrecisionInteger::from(-9)),
        ] {
            let compact = if exact.cmp(&Integer::Single(0)) == Ordering::Less {
                -9
            } else {
                7
            };
            assert_eq!(
                context
                    .integer_exact(&exact, ParametricArithmeticLimits::default())
                    .unwrap(),
                context.integer(compact),
            );
        }

        let huge = (Integer::one() << 4096_u32) + Integer::from(19);
        let huge_bits = usize::try_from(integer_magnitude_bits(&huge)).unwrap();
        assert!(huge_bits > i64::BITS as usize);
        let mut exact = ParametricArithmeticLimits::default();
        exact.max_specialization_integer_bits = huge_bits;
        let lifted = context.integer_exact(&huge, exact).unwrap();
        assert_eq!(lifted.raw.numerator.coefficients, [huge.clone()]);
        assert!(lifted.raw.numerator.is_constant());
        assert!(lifted.raw.denominator.is_one());

        let negative_huge = -huge.clone();
        let lifted_negative = context.integer_exact(&negative_huge, exact).unwrap();
        assert_eq!(lifted_negative.raw.numerator.coefficients, [negative_huge],);
        assert!(lifted_negative.raw.numerator.is_constant());
        assert!(lifted_negative.raw.denominator.is_one());

        exact.max_specialization_integer_bits = huge_bits - 1;
        assert_eq!(
            context.integer_exact(&huge, exact),
            Err(ParametricCoefficientError::ResourceLimit {
                resource: "exact integer constant bits",
                requested: huge_bits,
                limit: huge_bits - 1,
            })
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
                    operation: crate::algebra::ExactAlgebraOperation::Authenticate,
                    requested: 1,
                    limit: 0,
                    ..
                }
            ))
        ));
    }
}
