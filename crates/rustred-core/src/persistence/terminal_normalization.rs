//! Native finite-normalization sidecars. Import never establishes IBP closure.

use std::collections::BTreeSet;
use std::io::Write;

use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_normalization::{
    TerminalNormalizationError, TerminalNormalizationLimits, TerminalNormalizationPlan,
};
use crate::sector::OrderingPolicy;

use super::error::check_limit;
use super::limits::CappedWriter;
use super::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, BinarySection, CoefficientId,
    CoefficientTableBuilder, DecodedCoefficientTable, SectionTag, encode_program, inspect_program,
    same_native_coefficient,
};

const SCHEMA: u32 = 1;

pub(crate) fn encode(
    plan: &TerminalNormalizationPlan,
    limits: BinaryIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    let arity = plan
        .raw_terminals()
        .iter()
        .next()
        .map_or(0, |key| key.powers().len());
    check_counts(arity, plan.raw_terminals().len(), limits)?;
    let mut records = CappedWriter::new("normalization records", limits.max_program_bytes);
    scalar(&mut records, &SCHEMA)?;
    scalar(&mut records, &(arity as u64))?;
    bytes(&mut records, plan.family_fingerprint().as_bytes())?;
    bytes(&mut records, plan.ordering().stable_id().as_bytes())?;
    scalar(&mut records, &(plan.raw_terminals().len() as u64))?;
    let keys: Vec<_> = plan.raw_terminals().iter().collect();
    let mut coefficients = CoefficientTableBuilder::new(limits);
    let mut entries = 0usize;
    for key in &keys {
        for power in key.powers() {
            scalar(&mut records, power)?;
        }
        let row = &plan.terms()[*key];
        entries = entries
            .checked_add(row.len())
            .ok_or(BinaryIoError::Invalid("normalization entry count overflow"))?;
        check_limit(
            "normalization entries",
            entries,
            limits.max_collection_entries,
        )?;
        scalar(&mut records, &(row.len() as u64))?;
        for (output, coefficient) in row {
            let id = keys
                .binary_search(&output)
                .map_err(|_| BinaryIoError::Invalid("normalization output not declared"))?;
            scalar(&mut records, &(id as u64))?;
            scalar(
                &mut records,
                &(coefficients.intern(coefficient)?.index() as u64),
            )?;
        }
    }
    let coefficients = coefficients.finish()?;
    encode_program(
        BinaryProgramKind::TerminalNormalization,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &coefficients.state,
            },
            BinarySection {
                tag: SectionTag::COEFFICIENTS,
                bytes: &coefficients.atoms,
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &records.finish(),
            },
        ],
        limits,
    )
}

pub(crate) fn decode(
    bytes: &[u8],
    family: &IntegralFamily,
    raw: &BTreeSet<IntegralKey>,
    ordering: OrderingPolicy,
    preparation: TerminalNormalizationLimits,
    limits: BinaryIoLimits,
) -> Result<TerminalNormalizationPlan, TerminalNormalizationError> {
    check_counts(family.denominator_count(), raw.len(), limits)?;
    let envelope = inspect_program(bytes, limits)?;
    if envelope.kind() != BinaryProgramKind::TerminalNormalization
        || envelope.sections().iter().map(|s| s.tag).ne([
            SectionTag::SYMBOLICA_STATE,
            SectionTag::COEFFICIENTS,
            SectionTag::PROGRAM,
        ])
    {
        return Err(BinaryIoError::Invalid("program is not finite terminal normalization").into());
    }
    let mut records = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    if read::<u32>(&mut records)? != SCHEMA {
        return Err(BinaryIoError::Invalid("unsupported terminal normalization schema").into());
    }
    let arity = usize_value(&mut records)?;
    // Empty raw sets are represented by arity zero and carry no key records.
    if arity
        != if raw.is_empty() {
            0
        } else {
            family.denominator_count()
        }
    {
        return Err(BinaryIoError::Invalid("normalization arity mismatch").into());
    }
    if read_bytes(&mut records)? != family.fingerprint().as_bytes() {
        return Err(BinaryIoError::Invalid("normalization family mismatch").into());
    }
    if read_bytes(&mut records)? != ordering.stable_id().as_bytes() {
        return Err(BinaryIoError::Invalid("normalization ordering mismatch").into());
    }
    let count = usize_value(&mut records)?;
    if count != raw.len() {
        return Err(BinaryIoError::Invalid("normalization raw terminal count mismatch").into());
    }
    let minimum = arity
        .checked_add(1)
        .and_then(|n| n.checked_mul(count))
        .ok_or(BinaryIoError::Invalid("normalization size overflow"))?;
    if minimum > records.len() {
        return Err(BinaryIoError::Invalid("normalization record count exceeds bytes").into());
    }
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| allocation("normalization rows", count))?;
    let mut entries = 0usize;
    let mut next_coefficient = 0usize;
    for expected in raw {
        for &power in expected.powers() {
            if read::<i64>(&mut records)? != power {
                return Err(
                    BinaryIoError::Invalid("normalization raw terminal set mismatch").into(),
                );
            }
        }
        let nterms = usize_value(&mut records)?;
        entries = entries
            .checked_add(nterms)
            .ok_or(BinaryIoError::Invalid("normalization entry count overflow"))?;
        check_limit(
            "normalization entries",
            entries,
            limits.max_collection_entries,
        )?;
        if nterms > count || nterms > records.len() / 2 {
            return Err(
                BinaryIoError::Invalid("normalization output count exceeds keys or bytes").into(),
            );
        }
        let mut row = Vec::new();
        row.try_reserve_exact(nterms)
            .map_err(|_| allocation("normalization row terms", nterms))?;
        let mut previous = None;
        for _ in 0..nterms {
            let key = usize_value(&mut records)?;
            let coefficient = usize_value(&mut records)?;
            if key >= count || previous.is_some_and(|p| p >= key) {
                return Err(BinaryIoError::Invalid(
                    "normalization output IDs not strictly ordered",
                )
                .into());
            }
            if coefficient > next_coefficient {
                return Err(BinaryIoError::Invalid(
                    "normalization coefficient IDs not first-occurrence ordered",
                )
                .into());
            }
            if coefficient == next_coefficient {
                next_coefficient =
                    next_coefficient
                        .checked_add(1)
                        .ok_or(BinaryIoError::Invalid(
                            "normalization coefficient count overflow",
                        ))?;
            }
            row.push((key, CoefficientId::try_from_index(coefficient)?));
            previous = Some(key);
        }
        rows.push(row);
    }
    if !records.is_empty() {
        return Err(BinaryIoError::Invalid("trailing normalization records").into());
    }
    let coefficients = DecodedCoefficientTable::import_generated(
        envelope
            .section(SectionTag::SYMBOLICA_STATE)
            .expect("checked section"),
        envelope
            .section(SectionTag::COEFFICIENTS)
            .expect("checked section"),
        limits,
    )?;
    if coefficients.len() != next_coefficient {
        return Err(BinaryIoError::Invalid("unused or missing normalization coefficient").into());
    }
    for index in 0..coefficients.len() {
        family
            .coefficient_context()
            .validate_with_limits(
                coefficients.coefficient(CoefficientId::try_from_index(index)?)?,
                limits.exact_algebra,
            )
            .map_err(|e| BinaryIoError::Native(e.to_string()))?;
    }
    // Return the independently regenerated proof owner, not unchecked imported
    // coefficients. Structural bounds are not a hostile-native-parser promise.
    let regenerated =
        TerminalNormalizationPlan::vacuum_quadratic_numerators(family, raw, ordering, preparation)?;
    if regenerated.terms().len() != rows.len() {
        return Err(BinaryIoError::Invalid("regenerated normalization row count mismatch").into());
    }
    let keys: Vec<_> = raw.iter().collect();
    for ((source, expected), row) in regenerated.terms().iter().zip(&rows) {
        if expected.len() != row.len() {
            return Err(
                BinaryIoError::Invalid("normalization differs from regenerated proof").into(),
            );
        }
        for ((key, value), (key_id, coefficient_id)) in expected.iter().zip(row) {
            if key != keys[*key_id]
                || !same_native_coefficient(value, coefficients.coefficient(*coefficient_id)?)
            {
                return Err(
                    BinaryIoError::Invalid("normalization differs from regenerated proof").into(),
                );
            }
        }
        if !raw.contains(source) {
            return Err(BinaryIoError::Invalid("regenerated normalization source mismatch").into());
        }
    }
    Ok(regenerated)
}

fn check_counts(arity: usize, count: usize, limits: BinaryIoLimits) -> Result<(), BinaryIoError> {
    check_limit("normalization arity", arity, limits.max_collection_entries)?;
    check_limit("normalization rows", count, limits.max_collection_entries)?;
    let cells = arity.checked_mul(count).ok_or(BinaryIoError::Invalid(
        "normalization coordinate count overflow",
    ))?;
    check_limit(
        "normalization coordinates",
        cells,
        limits.max_collection_entries,
    )
}
fn scalar<T: bincode::Encode>(out: &mut impl Write, value: &T) -> Result<(), BinaryIoError> {
    bincode::encode_into_std_write(value, out, bincode::config::standard())
        .map(|_| ())
        .map_err(native)
}
fn read<T: bincode::Decode<()>>(source: &mut &[u8]) -> Result<T, BinaryIoError> {
    let (value, n) =
        bincode::decode_from_slice(source, bincode::config::standard()).map_err(native)?;
    *source = &source[n..];
    Ok(value)
}
fn usize_value(source: &mut &[u8]) -> Result<usize, BinaryIoError> {
    usize::try_from(read::<u64>(source)?).map_err(native)
}
fn bytes(out: &mut impl Write, value: &[u8]) -> Result<(), BinaryIoError> {
    scalar(out, &(value.len() as u64))?;
    out.write_all(value).map_err(native)
}
fn read_bytes<'a>(source: &mut &'a [u8]) -> Result<&'a [u8], BinaryIoError> {
    let size = usize_value(source)?;
    let value = source
        .get(..size)
        .ok_or(BinaryIoError::Invalid("truncated normalization string"))?;
    *source = &source[size..];
    Ok(value)
}
fn native(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}
fn allocation(resource: &'static str, requested: usize) -> BinaryIoError {
    BinaryIoError::Allocation {
        resource,
        requested,
    }
}
