//! Reconstruct bounded complete vacuum families from their actual inputs.
//! No named topology factory or source/rule discovery runs during decoding.

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, IntegralFamily};

use super::super::error::{ArtifactError, ArtifactPersistenceError};
use super::binary::{Reader, check_limit, try_vec};
use super::coefficient::decode_base_coefficient;
use super::{decode_owned_string, decode_strings};

#[derive(Clone, Copy)]
pub(super) enum FamilyGrammar {
    OneLoop,
    CompleteVacuum { arity: usize },
}

pub(super) fn decode(
    reader: &mut Reader<'_>,
    grammar: FamilyGrammar,
) -> Result<IntegralFamily, ArtifactPersistenceError> {
    let loop_count = reader.count("loop momentum labels")?;
    let external_count = reader.count("external momentum labels")?;
    let parameter_count = reader.count("coefficient parameter names")?;
    let denominator_count = reader.count("family denominators")?;
    let gram_rows = reader.count("external Gram rows")?;
    let power_shift_count = reader.count("family power shifts")?;
    let invalid = || ArtifactPersistenceError::SemanticMismatch {
        field: match grammar {
            FamilyGrammar::OneLoop => "one-loop family structural prelude",
            FamilyGrammar::CompleteVacuum { .. } => "complete vacuum family structural prelude",
        },
    };
    let arity = match grammar {
        FamilyGrammar::OneLoop => {
            if loop_count != 1 {
                return Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "one-loop family structural prelude",
                });
            }
            1
        }
        FamilyGrammar::CompleteVacuum { arity } => arity,
    };
    let coordinates = loop_count
        .checked_add(1)
        .and_then(|next| loop_count.checked_mul(next))
        .map(|square| square / 2)
        .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
            resource: "vacuum scalar product coordinates",
        })?;
    if loop_count == 0
        || external_count != 0
        || parameter_count != 1
        || gram_rows != 0
        || denominator_count != arity
        || power_shift_count != arity
        || coordinates != arity
    {
        return Err(invalid());
    }
    check_limit("artifact arity", arity, reader.limits().max_index_arity)?;
    check_limit(
        "family scalar products",
        coordinates,
        reader.limits().family.max_scalar_products,
    )?;
    let matrix_entries =
        arity
            .checked_mul(coordinates)
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
                resource: "family matrix entries",
            })?;
    check_limit(
        "family matrix entries",
        matrix_entries,
        reader.limits().family.max_matrix_entries,
    )?;
    check_limit(
        "family coefficient cells",
        matrix_entries,
        reader.limits().max_collection_entries,
    )?;
    for _ in 0..denominator_count {
        if reader.count("denominator coefficients")? != coordinates {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: match grammar {
                    FamilyGrammar::OneLoop => "one-loop denominator shape",
                    FamilyGrammar::CompleteVacuum { .. } => "complete vacuum denominator shape",
                },
            });
        }
    }
    let name = decode_owned_string(reader, "family name")?;
    let loop_momenta = decode_strings(reader, loop_count, "loop momentum labels")?;
    let parameters = decode_strings(reader, parameter_count, "coefficient parameter names")?;
    let context = CoefficientContext::try_new(parameters).map_err(ArtifactError::from)?;
    let dimension = decode_base_coefficient(reader, &context, "family dimension")?;
    let mut denominators = try_vec(denominator_count, "family denominators")?;
    for _ in 0..denominator_count {
        let constant = decode_base_coefficient(reader, &context, "denominator constant")?;
        let mut coefficients = try_vec(coordinates, "denominator coefficients")?;
        for _ in 0..coordinates {
            coefficients.push(decode_base_coefficient(
                reader,
                &context,
                "denominator coefficient",
            )?);
        }
        denominators.push(AffineDenominator::new(constant, coefficients));
    }
    let mut shifts = try_vec(power_shift_count, "family power shifts")?;
    for _ in 0..power_shift_count {
        shifts.push(decode_base_coefficient(
            reader,
            &context,
            "family power shift",
        )?);
    }
    IntegralFamily::new_with_limits(
        name,
        loop_momenta,
        Vec::new(),
        context,
        dimension,
        denominators,
        Vec::new(),
        shifts,
        reader.limits().family,
    )
    .map_err(ArtifactError::from)
    .map_err(ArtifactPersistenceError::from)
}

#[cfg(test)]
#[path = "family/tests.rs"]
mod tests;
