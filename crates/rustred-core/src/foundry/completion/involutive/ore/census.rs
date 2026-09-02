use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficient;

use crate::foundry::completion::involutive::error::{check_limit, checked_add, checked_mul};
use crate::foundry::completion::involutive::{InvolutiveError, InvolutiveLimits};

use super::model::{ConsequenceProvenance, OreRow};

/// Logical sparse payload retained by all row and provenance coefficients.
///
/// Byte counts exclude allocator metadata and spare capacity; entry/cell caps
/// remain the allocation-independent authority.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CoefficientPayloadCensus {
    terms: usize,
    exponent_cells: usize,
    retained_bytes: usize,
}

impl CoefficientPayloadCensus {
    pub(crate) const fn terms(self) -> usize {
        self.terms
    }

    pub(crate) const fn exponent_cells(self) -> usize {
        self.exponent_cells
    }

    pub(crate) const fn retained_bytes(self) -> usize {
        self.retained_bytes
    }

    pub(crate) fn try_add(self, right: Self) -> Result<Self, InvolutiveError> {
        Ok(Self {
            terms: checked_add("Ore coefficient payload terms", self.terms, right.terms)?,
            exponent_cells: checked_add(
                "Ore coefficient payload exponent cells",
                self.exponent_cells,
                right.exponent_cells,
            )?,
            retained_bytes: checked_add(
                "Ore coefficient payload retained bytes",
                self.retained_bytes,
                right.retained_bytes,
            )?,
        })
    }

    fn try_require_consequence_limits(
        self,
        limits: InvolutiveLimits,
    ) -> Result<Self, InvolutiveError> {
        check_limit(
            "Ore consequence coefficient terms",
            self.terms,
            limits.max_consequence_coefficient_terms,
        )?;
        check_limit(
            "Ore consequence coefficient exponent cells",
            self.exponent_cells,
            limits.max_consequence_coefficient_exponent_cells,
        )?;
        check_limit(
            "Ore consequence coefficient retained bytes",
            self.retained_bytes,
            limits.max_consequence_coefficient_retained_bytes,
        )?;
        Ok(self)
    }
}

pub(super) fn coefficient_payload_census(
    row: &OreRow,
    provenance: &ConsequenceProvenance,
    limits: InvolutiveLimits,
) -> Result<CoefficientPayloadCensus, InvolutiveError> {
    let mut census = CoefficientPayloadCensus::default();
    for coefficient in row
        .terms
        .iter()
        .map(|term| &term.coefficient)
        .chain(provenance.terms.iter().map(|term| &term.left_coefficient))
    {
        census = census.try_add(single_coefficient_census(coefficient)?)?;
    }
    census.try_require_consequence_limits(limits)
}

fn single_coefficient_census(
    coefficient: &IndexedCoefficient,
) -> Result<CoefficientPayloadCensus, InvolutiveError> {
    let mut census = CoefficientPayloadCensus {
        retained_bytes: std::mem::size_of::<IndexedCoefficient>(),
        ..CoefficientPayloadCensus::default()
    };
    for polynomial in [&coefficient.raw().numerator, &coefficient.raw().denominator] {
        census.terms = checked_add(
            "Ore coefficient payload terms",
            census.terms,
            polynomial.coefficients.len(),
        )?;
        census.exponent_cells = checked_add(
            "Ore coefficient payload exponent cells",
            census.exponent_cells,
            polynomial.exponents.len(),
        )?;
        census.retained_bytes = checked_add(
            "Ore coefficient payload retained bytes",
            census.retained_bytes,
            checked_add(
                "Ore coefficient payload retained bytes",
                checked_mul(
                    "Ore coefficient payload retained bytes",
                    polynomial.coefficients.len(),
                    std::mem::size_of::<Integer>(),
                )?,
                checked_mul(
                    "Ore coefficient payload retained bytes",
                    polynomial.exponents.len(),
                    std::mem::size_of::<u16>(),
                )?,
            )?,
        )?;
        for integer in &polynomial.coefficients {
            let Integer::Large(value) = integer else {
                continue;
            };
            let bits = usize::try_from(value.significant_bits()).map_err(|_| {
                InvolutiveError::ResourceCountOverflow {
                    resource: "Ore coefficient payload retained bytes",
                }
            })?;
            census.retained_bytes = checked_add(
                "Ore coefficient payload retained bytes",
                census.retained_bytes,
                checked_add("Ore coefficient payload retained bytes", bits, 7)? / 8,
            )?;
        }
    }
    Ok(census)
}
