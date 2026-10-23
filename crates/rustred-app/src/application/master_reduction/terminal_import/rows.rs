//! Trusted generated exact equation payloads. Symbolica owns coefficient bytes
//! and state remapping; this container only binds signed keys to atom IDs.
use std::collections::{BTreeMap, BTreeSet};

use rustred::algebra::Coefficient;
use rustred::family::{IntegralFamily, IntegralKey};
use rustred::persistence::{
    BinaryProgramKind, BinarySection, CoefficientId, CoefficientTableBuilder,
    DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
};
use rustred::reduction::terminal_relations::TerminalEquation;

use crate::AppError;

const SECTIONS: [SectionTag; 3] = [
    SectionTag::SYMBOLICA_STATE,
    SectionTag::COEFFICIENTS,
    SectionTag::PROGRAM,
];

#[derive(bincode::Encode, bincode::Decode)]
struct Record {
    schema: u32,
    family_fingerprint: String,
    arity: u64,
    binding: String,
    rows: Vec<Row>,
}

#[derive(bincode::Encode, bincode::Decode)]
struct Row {
    terms: Vec<(Vec<i64>, u32)>,
    conditions: Vec<u32>,
}

fn invalid(message: impl std::fmt::Display) -> AppError {
    AppError::schema(format!("terminal equation payload: {message}"))
}

fn validate_coefficient(
    family: &IntegralFamily,
    coefficient: &Coefficient,
) -> Result<(), AppError> {
    family
        .coefficient_context()
        .validate_with_limits(
            coefficient,
            super::super::profile::native_io_limits(None).exact_algebra,
        )
        .map_err(invalid)?;
    if coefficient.is_zero() {
        return Err(invalid("zero term or nonzero condition"));
    }
    Ok(())
}

pub(in crate::application::master_reduction) fn encode(
    family: &IntegralFamily,
    rows: &[TerminalEquation],
    binding: &str,
) -> Result<Vec<u8>, AppError> {
    if binding.is_empty() {
        return Err(invalid("empty authority binding"));
    }
    let limits = super::super::profile::native_io_limits(None);
    let mut table = CoefficientTableBuilder::new(limits);
    let mut intern = |coefficient: &Coefficient| -> Result<u32, AppError> {
        validate_coefficient(family, coefficient)?;
        u32::try_from(table.intern(coefficient).map_err(invalid)?.index()).map_err(invalid)
    };
    let rows = rows
        .iter()
        .map(|row| {
            let terms = row
                .terms
                .iter()
                .map(|(key, coefficient)| {
                    if key.powers().len() != family.denominator_count() {
                        return Err(invalid("integral-key arity mismatch"));
                    }
                    Ok((key.powers().to_vec(), intern(coefficient)?))
                })
                .collect::<Result<_, AppError>>()?;
            let conditions = row
                .nonzero_conditions
                .iter()
                .map(&mut intern)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(Row { terms, conditions })
        })
        .collect::<Result<_, AppError>>()?;
    let record = Record {
        schema: 1,
        family_fingerprint: family.fingerprint().into(),
        arity: u64::try_from(family.denominator_count()).map_err(invalid)?,
        binding: binding.into(),
        rows,
    };
    let program = bincode::encode_to_vec(record, bincode::config::standard())
        .map_err(|error| AppError::serialization(error.to_string()))?;
    let table = table.finish().map_err(invalid)?;
    encode_program(
        BinaryProgramKind::TerminalRelations,
        &[
            BinarySection {
                tag: SECTIONS[0],
                bytes: &table.state,
            },
            BinarySection {
                tag: SECTIONS[1],
                bytes: &table.atoms,
            },
            BinarySection {
                tag: SECTIONS[2],
                bytes: &program,
            },
        ],
        limits,
    )
    .map_err(invalid)
}

/// Not an untrusted-input parser or an independent source-identity proof. The
/// caller authenticates the payload's provenance before this generated import.
pub(in crate::application::master_reduction) fn decode(
    bytes: &[u8],
    family: &IntegralFamily,
    binding: &str,
) -> Result<Vec<TerminalEquation>, AppError> {
    let limits = super::super::profile::native_io_limits(None);
    let envelope = inspect_program(bytes, limits).map_err(invalid)?;
    if envelope.kind() != BinaryProgramKind::TerminalRelations
        || envelope
            .sections()
            .iter()
            .map(|section| section.tag)
            .ne(SECTIONS)
    {
        return Err(invalid("unexpected envelope kind or sections"));
    }
    let data = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    // Same trusted-generated boundary as native checkpoint records. A hidden
    // bincode allocation ceiling would reject payloads admitted by our writer.
    let (record, consumed): (Record, usize) =
        bincode::decode_from_slice(data, bincode::config::standard()).map_err(invalid)?;
    if consumed != data.len()
        || record.schema != 1
        || record.family_fingerprint != family.fingerprint()
        || record.arity != u64::try_from(family.denominator_count()).map_err(invalid)?
        || record.binding.is_empty()
        || record.binding != binding
    {
        return Err(invalid(
            "schema, family, arity, authority binding or trailing bytes mismatch",
        ));
    }
    let table = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked section"),
        envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section"),
        limits,
    )
    .map_err(invalid)?;
    let mut used = BTreeSet::new();
    let mut coefficient = |id: u32| -> Result<Coefficient, AppError> {
        // The writer assigns atom IDs in first-use order. Check that canonical
        // structural order as well as membership and use of every table entry.
        if !used.contains(&id) && id as usize != used.len() {
            return Err(invalid("noncanonical first-use coefficient ID"));
        }
        let value = table
            .coefficient(CoefficientId::try_from_index(id as usize).map_err(invalid)?)
            .map_err(invalid)?;
        validate_coefficient(family, value)?;
        used.insert(id);
        Ok(value.clone())
    };
    let rows = record
        .rows
        .into_iter()
        .map(|row| {
            let mut terms = BTreeMap::new();
            for (powers, id) in row.terms {
                if powers.len() != family.denominator_count() {
                    return Err(invalid("integral-key arity mismatch"));
                }
                let key = IntegralKey::try_new(powers).map_err(invalid)?;
                if terms
                    .last_key_value()
                    .is_some_and(|(previous, _)| previous >= &key)
                {
                    return Err(invalid("duplicate or unordered integral terms"));
                }
                terms.insert(key, coefficient(id)?);
            }
            let mut nonzero_conditions = Vec::new();
            for id in row.conditions {
                nonzero_conditions.push(coefficient(id)?);
            }
            Ok(TerminalEquation {
                terms,
                nonzero_conditions,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if used.len() != table.len() {
        return Err(invalid("unreferenced coefficient table entries"));
    }
    Ok(rows)
}

#[cfg(test)]
#[path = "rows_tests.rs"]
mod tests;
