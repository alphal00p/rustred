//! Aggregate retained-payload admission for the cold prototype.

use std::collections::BTreeMap;
use std::mem::size_of;

use symbolica::prelude::Integer;

use crate::algebra::{
    Coefficient, CoefficientContext, coefficient_clone_owned_retained_byte_bound,
};
use crate::family::IntegralKey;

use super::compile::admit_limit;
use super::error::FactorizedProductMomentError;
use super::limits::FactorizedProductMomentLimits;

const CONSERVATIVE_GMP_CAPACITY_FACTOR: usize = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct CoefficientWeight {
    terms: usize,
    clone_owned_bytes: usize,
}

impl CoefficientWeight {
    fn checked_add(self, other: Self) -> Result<Self, FactorizedProductMomentError> {
        Ok(Self {
            terms: self.terms.checked_add(other.terms).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product retained coefficient terms",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_add(other.clone_owned_bytes)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product retained coefficient clone-owned bytes",
                })?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, FactorizedProductMomentError> {
        Ok(Self {
            terms: self.terms.checked_sub(other.terms).ok_or(
                FactorizedProductMomentError::Invariant {
                    detail: "the retained coefficient-term census underflowed",
                },
            )?,
            clone_owned_bytes: self
                .clone_owned_bytes
                .checked_sub(other.clone_owned_bytes)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the retained coefficient-byte census underflowed",
                })?,
        })
    }
}

pub(super) fn coefficient_weight(
    coefficient: &Coefficient,
) -> Result<CoefficientWeight, FactorizedProductMomentError> {
    let terms = coefficient
        .numerator
        .nterms()
        .checked_add(coefficient.denominator.nterms())
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product retained coefficient terms",
        })?;
    let clone_owned_bytes = coefficient_clone_owned_retained_byte_bound(coefficient).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product retained coefficient clone-owned bytes",
        },
    )?;
    Ok(CoefficientWeight {
        terms,
        clone_owned_bytes,
    })
}

/// Exact aggregate of coefficients retained by native polynomial input,
/// memoization maps, guards, and output maps. Temporary checks include the
/// already-retained aggregate, so old and replacement values coexist at every
/// transactional update boundary.
pub(super) struct CoefficientBudget {
    current: CoefficientWeight,
    limits: FactorizedProductMomentLimits,
}

impl CoefficientBudget {
    pub(super) fn new(limits: FactorizedProductMomentLimits) -> Self {
        Self {
            current: CoefficientWeight::default(),
            limits,
        }
    }

    pub(super) fn retain(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        self.retain_weight(coefficient_weight(coefficient)?)
    }

    pub(super) fn release(
        &mut self,
        coefficient: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        self.current = self.current.checked_sub(coefficient_weight(coefficient)?)?;
        Ok(())
    }

    pub(super) fn replace(
        &mut self,
        old: &Coefficient,
        new: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        let old = coefficient_weight(old)?;
        let new = coefficient_weight(new)?;
        // The arithmetic result exists before the old retained value is
        // replaced. Admit that live peak transactionally first.
        self.admit(self.current.checked_add(new)?)?;
        let prospective = self.current.checked_sub(old)?.checked_add(new)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    pub(super) fn admit_temporaries<'coefficient>(
        &self,
        coefficients: impl IntoIterator<Item = &'coefficient Coefficient>,
    ) -> Result<(), FactorizedProductMomentError> {
        let mut prospective = self.current;
        for coefficient in coefficients {
            prospective = prospective.checked_add(coefficient_weight(coefficient)?)?;
        }
        self.admit(prospective)
    }

    /// Admit a pre-native envelope for `rows` constant integer coefficients.
    /// `integer_bits` is a mathematical height bound (for example, from an
    /// affine-polynomial l1 norm), while the context-bound unit contributes
    /// the exact fixed polynomial/vector-map clone ownership.
    pub(super) fn admit_native_integer_envelope(
        &self,
        rows: usize,
        integer_bits: usize,
        context_unit: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        let terms =
            rows.checked_mul(2)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product projected native coefficient terms",
                })?;
        let magnitude_bytes = conservative_integer_capacity_bytes(integer_bits)?;
        let per_coefficient = coefficient_clone_owned_retained_byte_bound(context_unit)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product projected native coefficient bytes",
            })?
            .checked_add(magnitude_bytes)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product projected native coefficient bytes",
            })?;
        let clone_owned_bytes = rows.checked_mul(per_coefficient).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product projected native coefficient bytes",
            },
        )?;
        let projected = CoefficientWeight {
            terms,
            clone_owned_bytes,
        };
        self.admit(self.current.checked_add(projected)?)
    }

    /// Admit a pre-native envelope for constant rational coefficients. Both
    /// numerator and denominator are limb-rounded independently because the
    /// Symbolica rational field owns both arbitrary-precision integers.
    pub(super) fn admit_native_rational_envelope(
        &self,
        rows: usize,
        numerator_bits: usize,
        denominator_bits: usize,
        context_unit: &Coefficient,
    ) -> Result<(), FactorizedProductMomentError> {
        let terms =
            rows.checked_mul(2)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product projected native coefficient terms",
                })?;
        let magnitude_bytes = conservative_integer_capacity_bytes(numerator_bits)?
            .checked_add(conservative_integer_capacity_bytes(denominator_bits)?)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product projected native coefficient bytes",
            })?;
        let per_coefficient = coefficient_clone_owned_retained_byte_bound(context_unit)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product projected native coefficient bytes",
            })?
            .checked_add(magnitude_bytes)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product projected native coefficient bytes",
            })?;
        let projected = CoefficientWeight {
            terms,
            clone_owned_bytes: rows.checked_mul(per_coefficient).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product projected native coefficient bytes",
                },
            )?,
        };
        self.admit(self.current.checked_add(projected)?)
    }

    fn retain_weight(
        &mut self,
        weight: CoefficientWeight,
    ) -> Result<(), FactorizedProductMomentError> {
        let prospective = self.current.checked_add(weight)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    fn admit(&self, weight: CoefficientWeight) -> Result<(), FactorizedProductMomentError> {
        admit_limit(
            "product retained coefficient terms",
            weight.terms,
            self.limits.max_retained_coefficient_terms,
        )?;
        admit_limit(
            "product retained coefficient clone-owned bytes",
            weight.clone_owned_bytes,
            self.limits.max_retained_coefficient_clone_owned_bytes,
        )
    }
}

/// Conservative owned capacity for one projected arbitrary-precision integer.
///
/// Symbolica's exact post-construction census uses the backend integer's
/// retained capacity.  Before construction, mirror the affine-input boundary:
/// round to a machine limb, charge one spare limb, then double the result for
/// allocator growth.  This intentionally overcharges inline `i64`/`i128`
/// values so the same bound remains safe across the representation boundary.
fn conservative_integer_capacity_bytes(
    integer_bits: usize,
) -> Result<usize, FactorizedProductMomentError> {
    let limb_bits = usize::BITS as usize;
    let rounded_bits = integer_bits
        .checked_add(limb_bits - 1)
        .and_then(|value| (value / limb_bits).checked_mul(limb_bits))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product projected native coefficient bytes",
        })?;
    let capacity_bits = rounded_bits
        .checked_add(limb_bits)
        .and_then(|value| value.checked_mul(CONSERVATIVE_GMP_CAPACITY_FACTOR))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product projected native coefficient bytes",
        })?;
    Ok(capacity_bits / 8)
}

pub(super) fn admit_dense_payload(
    entry_resource: &'static str,
    byte_resource: &'static str,
    rows: usize,
    width: usize,
    entry_bytes: usize,
    owner_bytes: usize,
    max_entries: usize,
    max_bytes: usize,
) -> Result<(), FactorizedProductMomentError> {
    let entries =
        rows.checked_mul(width)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: entry_resource,
            })?;
    admit_limit(entry_resource, entries, max_entries)?;
    let payload_bytes = entries.checked_mul(entry_bytes).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: byte_resource,
        },
    )?;
    let owner_bytes = rows.checked_mul(owner_bytes).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: byte_resource,
        },
    )?;
    let bytes = payload_bytes.checked_add(owner_bytes).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: byte_resource,
        },
    )?;
    admit_limit(byte_resource, bytes, max_bytes)
}

pub(super) fn admit_exponent_payload(
    rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    admit_dense_payload(
        "product polynomial exponent entries",
        "product polynomial exponent bytes",
        rows,
        width,
        size_of::<u32>(),
        0,
        limits.max_exponent_entries,
        limits.max_exponent_bytes,
    )
}

pub(super) fn admit_state_key_payload(
    rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    admit_dense_payload(
        "product moment state-key entries",
        "product moment state-key bytes",
        rows,
        width,
        size_of::<u64>(),
        size_of::<Box<[u64]>>(),
        limits.max_state_key_entries,
        limits.max_state_key_bytes,
    )
}

pub(super) fn admit_angular_order_key_payload(
    retained_rows: usize,
    order_rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let rows = retained_rows.checked_add(order_rows).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "angular retained state keys",
        },
    )?;
    let entries =
        rows.checked_mul(width)
            .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product moment state-key entries",
            })?;
    admit_limit(
        "product moment state-key entries",
        entries,
        limits.max_state_key_entries,
    )?;
    let payload_bytes = entries.checked_mul(size_of::<u64>()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        },
    )?;
    let retained_owner_bytes = retained_rows.checked_mul(size_of::<Box<[u64]>>()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        },
    )?;
    let order_owner_bytes = order_rows
        .checked_mul(size_of::<(usize, Box<[u64]>)>())
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        })?;
    let bytes = payload_bytes
        .checked_add(retained_owner_bytes)
        .and_then(|value| value.checked_add(order_owner_bytes))
        .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment state-key bytes",
        })?;
    admit_limit(
        "product moment state-key bytes",
        bytes,
        limits.max_state_key_bytes,
    )
}

pub(super) fn constant_integer_magnitude_bits(coefficient: &Coefficient) -> Option<usize> {
    if !coefficient.is_constant() || !coefficient.denominator.is_one() {
        return None;
    }
    constant_polynomial_magnitude_bits(&coefficient.numerator)
}

pub(super) fn constant_rational_magnitude_bits(
    coefficient: &Coefficient,
) -> Option<(usize, usize)> {
    if !coefficient.is_constant() {
        return None;
    }
    Some((
        constant_polynomial_magnitude_bits(&coefficient.numerator)?,
        constant_polynomial_magnitude_bits(&coefficient.denominator)?,
    ))
}

fn constant_polynomial_magnitude_bits(
    polynomial: &crate::algebra::CoefficientPolynomial,
) -> Option<usize> {
    if polynomial.is_zero() {
        return Some(0);
    }
    let [integer] = polynomial.coefficients.as_slice() else {
        return None;
    };
    let bits = match integer {
        Integer::Single(value) => u64::from(u64::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Double(value) => u64::from(u128::BITS - value.unsigned_abs().leading_zeros()),
        Integer::Large(value) => u64::from(value.significant_bits()),
    };
    usize::try_from(bits).ok()
}

pub(super) fn admit_output_key_payload(
    rows: usize,
    width: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    admit_dense_payload(
        "product output key power entries",
        "product output key bytes",
        rows,
        width,
        size_of::<i64>(),
        size_of::<IntegralKey>(),
        limits.max_output_key_power_entries,
        limits.max_output_key_bytes,
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct OutputKeyWeight {
    power_entries: usize,
    owner_and_power_bytes: usize,
}

impl OutputKeyWeight {
    fn for_key(key: &IntegralKey) -> Result<Self, FactorizedProductMomentError> {
        let power_entries = key.powers().len();
        let power_bytes = power_entries.checked_mul(size_of::<i64>()).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product retained output key bytes",
            },
        )?;
        let owner_and_power_bytes = power_bytes.checked_add(size_of::<IntegralKey>()).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product retained output key bytes",
            },
        )?;
        Ok(Self {
            power_entries,
            owner_and_power_bytes,
        })
    }

    fn checked_add(self, other: Self) -> Result<Self, FactorizedProductMomentError> {
        Ok(Self {
            power_entries: self.power_entries.checked_add(other.power_entries).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product retained output key power entries",
                },
            )?,
            owner_and_power_bytes: self
                .owner_and_power_bytes
                .checked_add(other.owner_and_power_bytes)
                .ok_or(FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product retained output key bytes",
                })?,
        })
    }

    fn checked_sub(self, other: Self) -> Result<Self, FactorizedProductMomentError> {
        Ok(Self {
            power_entries: self.power_entries.checked_sub(other.power_entries).ok_or(
                FactorizedProductMomentError::Invariant {
                    detail: "the retained output-key entry census underflowed",
                },
            )?,
            owner_and_power_bytes: self
                .owner_and_power_bytes
                .checked_sub(other.owner_and_power_bytes)
                .ok_or(FactorizedProductMomentError::Invariant {
                    detail: "the retained output-key byte census underflowed",
                })?,
        })
    }
}

/// Exact aggregate of every `IntegralKey` owned by a prototype map. This is
/// deliberately independent of each map's width limit: cache maps, returned
/// clones, convolution maps, and the final expansion coexist.
pub(super) struct OutputKeyBudget {
    current: OutputKeyWeight,
    limits: FactorizedProductMomentLimits,
}

impl OutputKeyBudget {
    pub(super) fn new(limits: FactorizedProductMomentLimits) -> Self {
        Self {
            current: OutputKeyWeight::default(),
            limits,
        }
    }

    pub(super) fn retain(&mut self, key: &IntegralKey) -> Result<(), FactorizedProductMomentError> {
        let prospective = self.current.checked_add(OutputKeyWeight::for_key(key)?)?;
        self.admit(prospective)?;
        self.current = prospective;
        Ok(())
    }

    pub(super) fn release(
        &mut self,
        key: &IntegralKey,
    ) -> Result<(), FactorizedProductMomentError> {
        self.current = self.current.checked_sub(OutputKeyWeight::for_key(key)?)?;
        Ok(())
    }

    /// Admit a newly allocated key that will coexist transiently with all
    /// currently retained map keys, without transferring ownership yet.
    pub(super) fn admit_temporary(
        &self,
        key: &IntegralKey,
    ) -> Result<(), FactorizedProductMomentError> {
        self.admit(self.current.checked_add(OutputKeyWeight::for_key(key)?)?)
    }

    fn admit(&self, weight: OutputKeyWeight) -> Result<(), FactorizedProductMomentError> {
        admit_limit(
            "product retained output key power entries",
            weight.power_entries,
            self.limits.max_output_key_power_entries,
        )?;
        admit_limit(
            "product retained output key bytes",
            weight.owner_and_power_bytes,
            self.limits.max_output_key_bytes,
        )
    }
}

pub(super) fn admit_guard_key_payload(
    rows: usize,
    limits: FactorizedProductMomentLimits,
) -> Result<(), FactorizedProductMomentError> {
    let bytes = rows.checked_mul(size_of::<(usize, u64)>()).ok_or(
        FactorizedProductMomentError::ResourceCountOverflow {
            resource: "product moment guard-key bytes",
        },
    )?;
    admit_limit(
        "product moment guard-key bytes",
        bytes,
        limits.max_guard_key_bytes,
    )
}

pub(super) fn accumulate_coefficient(
    context: &CoefficientContext,
    output: &mut BTreeMap<IntegralKey, Coefficient>,
    key: IntegralKey,
    coefficient: Coefficient,
    budget: &mut CoefficientBudget,
    key_budget: &mut OutputKeyBudget,
    limits: FactorizedProductMomentLimits,
    key_arity: usize,
    additions: &mut usize,
) -> Result<(), FactorizedProductMomentError> {
    // The caller has materialized this key before map dispatch. Account for
    // that live temporary even when the entry is occupied and the key is
    // immediately dropped.
    key_budget.admit_temporary(&key)?;
    if coefficient.is_zero() {
        return Ok(());
    }
    if !output.contains_key(&key) {
        let requested = output.len().checked_add(1).ok_or(
            FactorizedProductMomentError::ResourceCountOverflow {
                resource: "product output terms",
            },
        )?;
        admit_limit("product output terms", requested, limits.max_output_terms)?;
        // Per-map width remains useful for deterministic local bounds. The
        // aggregate budget below additionally covers all coexisting maps.
        admit_output_key_payload(requested, key_arity, limits)?;
    }
    match output.entry(key) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            budget.retain(&coefficient)?;
            key_budget.retain(entry.key())?;
            entry.insert(coefficient);
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => {
            let prospective_additions = additions.checked_add(1).ok_or(
                FactorizedProductMomentError::ResourceCountOverflow {
                    resource: "product coefficient coalescing additions",
                },
            )?;
            admit_limit(
                "product coefficient coalescing additions",
                prospective_additions,
                limits.max_coalescing_additions,
            )?;
            budget.admit_temporaries([&coefficient])?;
            let sum = context.try_add(entry.get(), &coefficient, limits.exact_algebra)?;
            budget.admit_temporaries([&coefficient, &sum])?;
            if sum.is_zero() {
                budget.release(entry.get())?;
                key_budget.release(entry.key())?;
                entry.remove();
            } else {
                budget.replace(entry.get(), &sum)?;
                *entry.get_mut() = sum;
            }
            *additions = prospective_additions;
        }
    }
    Ok(())
}

pub(super) fn release_map_resources(
    map: &BTreeMap<IntegralKey, Coefficient>,
    budget: &mut CoefficientBudget,
    key_budget: &mut OutputKeyBudget,
) -> Result<(), FactorizedProductMomentError> {
    for (key, coefficient) in map {
        budget.release(coefficient)?;
        key_budget.release(key)?;
    }
    Ok(())
}
