use std::mem::size_of;

use crate::algebra::{IndexedCoefficient, coefficient_clone_owned_retained_byte_bound};

use super::ModularGuideError;
use super::error::checked_add;

const PAYLOAD_TERMS: &str = "modular coefficient payload terms";
const PAYLOAD_EXPONENT_CELLS: &str = "modular coefficient payload exponent cells";
const PAYLOAD_BYTES: &str = "modular coefficient payload bytes";

/// Conservative owned payload weight shared by exact leaves and cold exact
/// materialization. Container-specific handles and lookup entries are added
/// by each owner separately.
#[derive(Clone, Copy, Debug)]
pub(super) struct CoefficientPayloadWeight {
    pub(super) terms: usize,
    pub(super) exponent_cells: usize,
    pub(super) bytes: usize,
}

pub(super) fn try_coefficient_payload_weight(
    coefficient: &IndexedCoefficient,
) -> Result<CoefficientPayloadWeight, ModularGuideError> {
    let raw = coefficient.raw();
    let terms = checked_add(
        PAYLOAD_TERMS,
        raw.numerator.coefficients.len(),
        raw.denominator.coefficients.len(),
    )?;
    let exponent_cells = checked_add(
        PAYLOAD_EXPONENT_CELLS,
        raw.numerator.exponents.len(),
        raw.denominator.exponents.len(),
    )?;
    let clone_owned = coefficient_clone_owned_retained_byte_bound(raw).ok_or(
        ModularGuideError::ResourceCountOverflow {
            resource: PAYLOAD_BYTES,
        },
    )?;
    // Conservatively count the complete authenticated wrapper in addition to
    // Symbolica's raw clone bound. This overcounts its inline raw header but
    // never understates retained memory.
    let bytes = checked_add(PAYLOAD_BYTES, clone_owned, size_of::<IndexedCoefficient>())?;
    Ok(CoefficientPayloadWeight {
        terms,
        exponent_cells,
        bytes,
    })
}
