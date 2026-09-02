use std::sync::Arc;

use symbolica::{domains::InternalOrdering, prelude::Integer};

use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};

use crate::foundry::completion::involutive::error::{
    check_limit, checked_add, checked_mul, try_push_bounded, try_vec,
};
use crate::foundry::completion::involutive::{InvolutiveError, InvolutiveLimits};

/// Canonical conjunction of nonzero conditions used by a derived consequence
/// or by discarded zero normal-form proofs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LocalizationWitness {
    guards: Box<[Arc<IndexedPolynomial>]>,
    census: LocalizationGuardCensus,
}

/// Logical sparse payload of a canonical localization witness.
///
/// Retained bytes exclude allocator metadata and spare capacity. The exact
/// count, term, and exponent-cell limits remain allocation-independent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct LocalizationGuardCensus {
    count: usize,
    terms: usize,
    exponent_cells: usize,
    retained_bytes: usize,
}

impl LocalizationWitness {
    pub(crate) fn guards(&self) -> &[Arc<IndexedPolynomial>] {
        &self.guards
    }

    pub(crate) fn census(&self) -> LocalizationGuardCensus {
        self.census
    }

    pub(crate) fn try_union(
        &self,
        other: &Self,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        self.clone()
            .try_merge_canonical_arcs(other.guards.iter().cloned(), limits)
    }

    pub(super) fn try_merge_polynomials(
        self,
        guards: Vec<IndexedPolynomial>,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        check_limit(
            "Ore incoming localization guards",
            guards.len(),
            limits
                .max_axpy_input_terms
                .max(limits.max_localization_guards),
        )?;
        let mut canonical = try_vec("canonical Ore localization guards", guards.len())?;
        for guard in guards {
            if let Some(guard) = try_canonical_nonzero_guard(context, &guard, limits)? {
                canonical.push(Arc::new(guard));
            }
        }
        self.try_merge_canonical_arcs(canonical, limits)
    }

    pub(super) fn try_merge_canonical_arcs(
        self,
        guards: impl IntoIterator<Item = Arc<IndexedPolynomial>>,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        let mut incoming = Vec::new();
        for guard in guards {
            try_push_bounded(
                &mut incoming,
                guard,
                "Ore incoming canonical localization guards",
                limits
                    .max_axpy_input_terms
                    .max(limits.max_localization_guards),
            )?;
        }
        incoming.sort_unstable_by(|left, right| left.raw().internal_cmp(right.raw()));
        incoming.dedup_by(|left, right| left == right);

        let existing = self.guards;
        let mut left = 0usize;
        let mut right = 0usize;
        let mut count = 0usize;
        let mut census = LocalizationGuardCensus::default();
        while left < existing.len() || right < incoming.len() {
            let selected = match (existing.get(left), incoming.get(right)) {
                (Some(left_guard), Some(right_guard)) => {
                    match left_guard.raw().internal_cmp(right_guard.raw()) {
                        std::cmp::Ordering::Less => {
                            left += 1;
                            left_guard
                        }
                        std::cmp::Ordering::Greater => {
                            right += 1;
                            right_guard
                        }
                        std::cmp::Ordering::Equal => {
                            left += 1;
                            right += 1;
                            left_guard
                        }
                    }
                }
                (Some(left_guard), None) => {
                    left += 1;
                    left_guard
                }
                (None, Some(right_guard)) => {
                    right += 1;
                    right_guard
                }
                (None, None) => break,
            };
            count = checked_add("Ore localization guards", count, 1)?;
            census = census.try_add(guard_census(selected)?, limits)?;
        }
        check_limit(
            "Ore localization guards",
            count,
            limits.max_localization_guards,
        )?;

        let mut merged = try_vec("merged Ore localization guards", count)?;
        left = 0;
        right = 0;
        while left < existing.len() || right < incoming.len() {
            match (existing.get(left), incoming.get(right)) {
                (Some(left_guard), Some(right_guard)) => {
                    match left_guard.raw().internal_cmp(right_guard.raw()) {
                        std::cmp::Ordering::Less => {
                            merged.push(Arc::clone(left_guard));
                            left += 1;
                        }
                        std::cmp::Ordering::Greater => {
                            merged.push(Arc::clone(right_guard));
                            right += 1;
                        }
                        std::cmp::Ordering::Equal => {
                            merged.push(Arc::clone(left_guard));
                            left += 1;
                            right += 1;
                        }
                    }
                }
                (Some(left_guard), None) => {
                    merged.push(Arc::clone(left_guard));
                    left += 1;
                }
                (None, Some(right_guard)) => {
                    merged.push(Arc::clone(right_guard));
                    right += 1;
                }
                (None, None) => break,
            }
        }
        Ok(Self {
            guards: merged.into_boxed_slice(),
            census,
        })
    }

    pub(super) fn try_validate(
        &self,
        context: &IndexedCoefficientContext,
        limits: InvolutiveLimits,
    ) -> Result<(), InvolutiveError> {
        let mut census = LocalizationGuardCensus::default();
        let mut previous: Option<&IndexedPolynomial> = None;
        for guard in &self.guards {
            context.validate_polynomial_with_limits(guard, limits.indexed_algebra.exact_algebra)?;
            if guard.is_zero() || guard.is_nonzero_constant() {
                return Err(InvolutiveError::Invariant {
                    detail: "Ore localization retained a zero or constant guard",
                });
            }
            if previous.is_some_and(|previous| previous.raw().internal_cmp(guard.raw()).is_ge()) {
                return Err(InvolutiveError::Invariant {
                    detail: "Ore localization guards are not canonical sorted unique",
                });
            }
            census = census.try_add(guard_census(guard)?, limits)?;
            previous = Some(guard);
        }
        if census != self.census {
            return Err(InvolutiveError::Invariant {
                detail: "Ore localization guard census disagrees with its payload",
            });
        }
        Ok(())
    }
}

impl LocalizationGuardCensus {
    pub(crate) const fn count(self) -> usize {
        self.count
    }

    pub(crate) const fn terms(self) -> usize {
        self.terms
    }

    pub(crate) const fn exponent_cells(self) -> usize {
        self.exponent_cells
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }

    fn try_add(self, right: Self, limits: InvolutiveLimits) -> Result<Self, InvolutiveError> {
        let result = Self {
            count: checked_add("Ore localization guards", self.count, right.count)?,
            terms: checked_add("Ore localization guard terms", self.terms, right.terms)?,
            exponent_cells: checked_add(
                "Ore localization guard exponent cells",
                self.exponent_cells,
                right.exponent_cells,
            )?,
            retained_bytes: checked_add(
                "Ore localization guard retained bytes",
                self.retained_bytes,
                right.retained_bytes,
            )?,
        };
        check_limit(
            "Ore localization guards",
            result.count,
            limits.max_localization_guards,
        )?;
        check_limit(
            "Ore localization guard terms",
            result.terms,
            limits.max_localization_guard_terms,
        )?;
        check_limit(
            "Ore localization guard exponent cells",
            result.exponent_cells,
            limits.max_localization_guard_exponent_cells,
        )?;
        check_limit(
            "Ore localization guard retained bytes",
            result.retained_bytes,
            limits.max_localization_guard_retained_bytes,
        )?;
        Ok(result)
    }
}

pub(super) fn try_canonical_nonzero_guard(
    context: &IndexedCoefficientContext,
    guard: &IndexedPolynomial,
    limits: InvolutiveLimits,
) -> Result<Option<IndexedPolynomial>, InvolutiveError> {
    context.validate_polynomial_with_limits(guard, limits.indexed_algebra.exact_algebra)?;
    if guard.is_zero() {
        return Err(InvolutiveError::Invariant {
            detail: "a zero polynomial cannot define a nonzero localization guard",
        });
    }
    if guard.is_nonzero_constant() {
        return Ok(None);
    }
    Ok(Some(context.primitive_guard_associate_with_limits(
        guard,
        limits.indexed_algebra.exact_algebra,
        limits.max_localization_guard_retained_bytes,
    )?))
}

fn guard_census(guard: &IndexedPolynomial) -> Result<LocalizationGuardCensus, InvolutiveError> {
    let polynomial = guard.raw();
    let terms = polynomial.coefficients.len();
    let exponent_cells = polynomial.exponents.len();
    let coefficient_slots = checked_mul(
        "Ore localization guard retained bytes",
        terms,
        std::mem::size_of::<Integer>(),
    )?;
    let exponent_bytes = checked_mul(
        "Ore localization guard retained bytes",
        exponent_cells,
        std::mem::size_of::<u16>(),
    )?;
    let mut retained_bytes = checked_add(
        "Ore localization guard retained bytes",
        checked_add(
            "Ore localization guard retained bytes",
            std::mem::size_of::<IndexedPolynomial>(),
            std::mem::size_of::<Arc<IndexedPolynomial>>(),
        )?,
        checked_add(
            "Ore localization guard retained bytes",
            coefficient_slots,
            exponent_bytes,
        )?,
    )?;
    for coefficient in &polynomial.coefficients {
        let large_bits = match coefficient {
            Integer::Large(value) => usize::try_from(value.significant_bits()).map_err(|_| {
                InvolutiveError::ResourceCountOverflow {
                    resource: "Ore localization guard retained bytes",
                }
            })?,
            Integer::Single(_) | Integer::Double(_) => 0,
        };
        let large_bytes = checked_add("Ore localization guard retained bytes", large_bits, 7)? / 8;
        retained_bytes = checked_add(
            "Ore localization guard retained bytes",
            retained_bytes,
            large_bytes,
        )?;
    }
    Ok(LocalizationGuardCensus {
        count: 1,
        terms,
        exponent_cells,
        retained_bytes,
    })
}
