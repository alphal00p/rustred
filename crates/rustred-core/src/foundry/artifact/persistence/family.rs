//! Reconstruct bounded complete vacuum families, and complete propagator-type
//! families with one external momentum, from their actual inputs.
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
    let (arity, max_externals) = match grammar {
        FamilyGrammar::OneLoop => {
            if loop_count != 1 {
                return Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "one-loop family structural prelude",
                });
            }
            (1, 0)
        }
        // The source-port audit admits at most one external momentum.
        FamilyGrammar::CompleteVacuum { arity } => (arity, 1),
    };
    let coordinates = loop_count
        .checked_add(1)
        .and_then(|next| loop_count.checked_mul(next))
        .map(|square| square / 2)
        .and_then(|loop_loop| {
            loop_count
                .checked_mul(external_count)
                .and_then(|loop_external| loop_loop.checked_add(loop_external))
        })
        .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
            resource: "vacuum scalar product coordinates",
        })?;
    if loop_count == 0
        || external_count > max_externals
        || parameter_count != 1
        || gram_rows != external_count
        || denominator_count != arity
        || power_shift_count != arity
        || coordinates != arity
    {
        return Err(invalid());
    }
    reader
        .limits()
        .check_count("artifact arity", arity, reader.limits().max_index_arity)?;
    let matrix_entries =
        arity
            .checked_mul(coordinates)
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow {
                resource: "family matrix entries",
            })?;
    if reader.limits().family.diagnostics_enabled {
        check_limit(
            "family scalar products",
            coordinates,
            reader.limits().family.max_scalar_products,
        )?;
        check_limit(
            "family matrix entries",
            matrix_entries,
            reader.limits().family.max_matrix_entries,
        )?;
    }
    reader.limits().check_count(
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
    for _ in 0..gram_rows {
        if reader.count("external Gram columns")? != external_count {
            return Err(invalid());
        }
    }
    let name = decode_owned_string(reader, "family name")?;
    let loop_momenta = decode_strings(reader, loop_count, "loop momentum labels")?;
    let external_momenta = decode_strings(reader, external_count, "external momentum labels")?;
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
    let mut gram = try_vec(gram_rows, "external Gram rows")?;
    for _ in 0..gram_rows {
        let mut row = try_vec(external_count, "external Gram columns")?;
        for _ in 0..external_count {
            row.push(decode_base_coefficient(
                reader,
                &context,
                "external Gram entry",
            )?);
        }
        gram.push(row);
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
        external_momenta,
        context,
        dimension,
        denominators,
        gram,
        shifts,
        reader.limits().family,
    )
    .map_err(ArtifactError::from)
    .map_err(ArtifactPersistenceError::from)
}

#[cfg(test)]
#[path = "family/tests.rs"]
mod tests;
