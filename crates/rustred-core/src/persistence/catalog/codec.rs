//! Structural catalog records plus Symbolica-owned state and expression frames.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::panic::{AssertUnwindSafe, catch_unwind};

use symbolica::atom::{Atom, AtomCore};
use symbolica::state::State;

use crate::family::IntegralKey;
use crate::persistence::error::check_limit;
use crate::persistence::limits::CappedWriter;
use crate::persistence::native::{
    BorrowedStateMap, NATIVE_ATOM_HEADER_BYTES, ensure_native_word_size, preflight_native_frame,
    read_length, write_length,
};
use crate::persistence::{
    BinaryIoError, BinaryIoLimits, BinaryProgramKind, BinarySection, SectionTag, encode_program,
    inspect_program,
};

use super::{
    ExactTerminalCatalog, TerminalCatalogCoverage, validate_exact_value, validate_identity,
};

const SCHEMA: u32 = 1;

pub(super) fn encode(
    catalog: &ExactTerminalCatalog,
    limits: BinaryIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    ensure_native_word_size()?;
    check_counts(catalog.index_count, catalog.terms.len(), limits)?;
    let mut records = CappedWriter::new("terminal catalog records", limits.max_program_bytes);
    scalar(&mut records, &SCHEMA)?;
    scalar(
        &mut records,
        &match catalog.coverage {
            TerminalCatalogCoverage::Partial => 0u8,
            TerminalCatalogCoverage::Complete => 1,
        },
    )?;
    scalar(&mut records, &(catalog.index_count as u64))?;
    scalar(&mut records, &(catalog.family_fingerprint.len() as u64))?;
    records
        .write_all(catalog.family_fingerprint.as_bytes())
        .map_err(native_error)?;
    scalar(&mut records, &(catalog.terms.len() as u64))?;

    // References borrow the immutable input. Neither the hash table nor the
    // dictionary clones a possibly large expression during encoding.
    let mut dictionary = Vec::<&Atom>::new();
    let mut ids = HashMap::<&Atom, u32>::new();
    for (key, value) in &catalog.terms {
        let id = if let Some(&id) = ids.get(value) {
            id
        } else {
            let id = u32::try_from(dictionary.len())
                .map_err(|_| BinaryIoError::Invalid("terminal value ID exceeds u32"))?;
            dictionary.try_reserve(1).map_err(|_| {
                allocation(
                    "terminal value dictionary",
                    dictionary.len().saturating_add(1),
                )
            })?;
            ids.try_reserve(1).map_err(|_| {
                allocation("terminal value dictionary", ids.len().saturating_add(1))
            })?;
            dictionary.push(value);
            ids.insert(value, id);
            id
        };
        for power in key.powers() {
            scalar(&mut records, power)?;
        }
        scalar(&mut records, &id)?;
    }

    let mut symbols = Atom::new().get_all_symbols(true);
    let mut values = CappedWriter::new("terminal value table bytes", limits.max_total_atom_bytes);
    write_length(&mut values, dictionary.len())?;
    for value in dictionary {
        symbols.extend(value.get_all_symbols(true));
        let frame_bytes = value
            .as_view()
            .get_data()
            .len()
            .checked_add(NATIVE_ATOM_HEADER_BYTES)
            .ok_or(BinaryIoError::Invalid(
                "terminal value frame length overflow",
            ))?;
        check_limit(
            "terminal value atom bytes",
            frame_bytes,
            limits.max_atom_bytes,
        )?;
        write_length(&mut values, frame_bytes)?;
        let written =
            bincode::encode_into_std_write(value, &mut values, bincode::config::standard())
                .map_err(native_error)?;
        if written != frame_bytes {
            return Err(BinaryIoError::Invalid(
                "native atom encoding length changed",
            ));
        }
    }
    let mut state = CappedWriter::new("Symbolica state bytes", limits.max_state_bytes);
    State::export_partial(&mut state, symbols).map_err(native_error)?;
    encode_program(
        BinaryProgramKind::TerminalValues,
        &[
            BinarySection {
                tag: SectionTag::SYMBOLICA_STATE,
                bytes: &state.finish(),
            },
            BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &records.finish(),
            },
            BinarySection {
                tag: SectionTag::VALUES,
                bytes: &values.finish(),
            },
        ],
        limits,
    )
}

pub(super) fn decode(
    bytes: &[u8],
    expected_family: &str,
    expected_arity: usize,
    limits: BinaryIoLimits,
) -> Result<ExactTerminalCatalog, BinaryIoError> {
    validate_identity(expected_family, expected_arity)?;
    let envelope = inspect_program(bytes, limits)?;
    if envelope.kind() != BinaryProgramKind::TerminalValues
        || envelope.sections().iter().map(|s| s.tag).ne([
            SectionTag::SYMBOLICA_STATE,
            SectionTag::PROGRAM,
            SectionTag::VALUES,
        ])
    {
        return Err(BinaryIoError::Invalid(
            "program is not an exact terminal-value catalog",
        ));
    }
    let mut records = envelope
        .section(SectionTag::PROGRAM)
        .expect("checked section");
    let schema: u32 = read_scalar(&mut records)?;
    if schema != SCHEMA {
        return Err(BinaryIoError::Invalid(
            "unsupported terminal catalog schema",
        ));
    }
    let coverage = match read_scalar::<u8>(&mut records)? {
        0 => TerminalCatalogCoverage::Partial,
        1 => TerminalCatalogCoverage::Complete,
        _ => return Err(BinaryIoError::Invalid("invalid terminal catalog coverage")),
    };
    let index_count = read_usize(&mut records)?;
    if index_count != expected_arity {
        return Err(BinaryIoError::Invalid("terminal catalog arity mismatch"));
    }
    let fingerprint_bytes = read_usize(&mut records)?;
    let fingerprint = take(&mut records, fingerprint_bytes)?;
    if fingerprint != expected_family.as_bytes() {
        return Err(BinaryIoError::Invalid(
            "terminal catalog family fingerprint mismatch",
        ));
    }
    let count = read_usize(&mut records)?;
    check_counts(index_count, count, limits)?;
    // Every coordinate and ID occupies at least one byte under bincode's
    // standard encoding. Reject impossible counts before reserving records.
    let minimum_bytes = index_count
        .checked_add(1)
        .and_then(|width| width.checked_mul(count))
        .ok_or(BinaryIoError::Invalid(
            "terminal catalog record size overflow",
        ))?;
    if minimum_bytes > records.len() {
        return Err(BinaryIoError::Invalid(
            "terminal record count exceeds remaining bytes",
        ));
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| allocation("terminal catalog entries", count))?;
    for _ in 0..count {
        let mut powers = Vec::new();
        powers
            .try_reserve_exact(index_count)
            .map_err(|_| allocation("terminal key coordinates", index_count))?;
        for _ in 0..index_count {
            powers.push(read_scalar::<i64>(&mut records)?);
        }
        let key = IntegralKey::try_new(powers).map_err(native_error)?;
        if entries.last().is_some_and(|(previous, _)| previous >= &key) {
            return Err(BinaryIoError::Invalid(
                "terminal keys are not strictly increasing",
            ));
        }
        entries.push((key, read_scalar::<u32>(&mut records)?));
    }
    if !records.is_empty() {
        return Err(BinaryIoError::Invalid("trailing terminal catalog records"));
    }

    let table = envelope
        .section(SectionTag::VALUES)
        .expect("checked section");
    check_limit(
        "terminal value table bytes",
        table.len(),
        limits.max_total_atom_bytes,
    )?;
    let mut source = table;
    let value_count = read_length(&mut source)?;
    check_limit(
        "terminal value table entries",
        value_count,
        limits.max_collection_entries,
    )?;
    if value_count > count || value_count > source.len() / (8 + NATIVE_ATOM_HEADER_BYTES + 1) {
        return Err(BinaryIoError::Invalid(
            "terminal value count exceeds records or frames",
        ));
    }
    let mut next_id = 0usize;
    for (_, id) in &entries {
        let id = *id as usize;
        if id >= value_count || id > next_id {
            return Err(BinaryIoError::Invalid(
                "terminal value ID is missing or not first-occurrence ordered",
            ));
        }
        if id == next_id {
            next_id += 1;
        }
    }
    if next_id != value_count {
        return Err(BinaryIoError::Invalid(
            "unused terminal value dictionary entry",
        ));
    }
    let mut frames = Vec::new();
    frames
        .try_reserve_exact(value_count)
        .map_err(|_| allocation("terminal native frames", value_count))?;
    for _ in 0..value_count {
        let size = read_length(&mut source)?;
        check_limit("terminal value atom bytes", size, limits.max_atom_bytes)?;
        let frame = take(&mut source, size)?;
        preflight_native_frame(frame)?;
        frames.push(frame);
    }
    if !source.is_empty() {
        return Err(BinaryIoError::Invalid(
            "trailing terminal value table bytes",
        ));
    }

    let state = envelope
        .section(SectionTag::SYMBOLICA_STATE)
        .expect("checked section");
    let state_map = catch_unwind(AssertUnwindSafe(|| {
        let mut source = state;
        let map = State::import(&mut source, None).map_err(native_error)?;
        if !source.is_empty() {
            return Err(BinaryIoError::Invalid("trailing Symbolica state bytes"));
        }
        Ok(map)
    }))
    .map_err(|_| BinaryIoError::Native("native Symbolica state import panicked".into()))??;
    let mut values = Vec::new();
    values
        .try_reserve_exact(value_count)
        .map_err(|_| allocation("decoded terminal values", value_count))?;
    let mut unique = HashSet::new();
    unique
        .try_reserve(value_count)
        .map_err(|_| allocation("decoded terminal value identities", value_count))?;
    for frame in frames {
        let value = catch_unwind(AssertUnwindSafe(|| {
            let (atom, consumed): (Atom, usize) = bincode::decode_from_slice_with_context(
                frame,
                bincode::config::standard(),
                BorrowedStateMap(&state_map),
            )
            .map_err(native_error)?;
            if consumed != frame.len() {
                return Err(BinaryIoError::Invalid("trailing native atom bytes"));
            }
            validate_exact_value(&atom)?;
            Ok(atom)
        }))
        .map_err(|_| BinaryIoError::Native("native terminal value import panicked".into()))??;
        if !unique.insert(value.clone()) {
            return Err(BinaryIoError::Invalid(
                "duplicate terminal value dictionary entry",
            ));
        }
        values.push(value);
    }
    let terms = entries
        .into_iter()
        .map(|(key, id)| (key, values[id as usize].clone()))
        .collect::<BTreeMap<_, _>>();
    Ok(ExactTerminalCatalog {
        family_fingerprint: expected_family.to_owned(),
        index_count,
        coverage,
        terms,
    })
}

fn scalar<T: bincode::Encode>(writer: &mut impl Write, value: &T) -> Result<(), BinaryIoError> {
    bincode::encode_into_std_write(value, writer, bincode::config::standard())
        .map(|_| ())
        .map_err(native_error)
}

fn read_scalar<T: bincode::Decode<()>>(source: &mut &[u8]) -> Result<T, BinaryIoError> {
    let (value, consumed) =
        bincode::decode_from_slice(source, bincode::config::standard()).map_err(native_error)?;
    *source = &source[consumed..];
    Ok(value)
}

fn read_usize(source: &mut &[u8]) -> Result<usize, BinaryIoError> {
    usize::try_from(read_scalar::<u64>(source)?)
        .map_err(|_| BinaryIoError::Invalid("catalog count exceeds host width"))
}

fn take<'a>(source: &mut &'a [u8], count: usize) -> Result<&'a [u8], BinaryIoError> {
    let value = source
        .get(..count)
        .ok_or(BinaryIoError::Invalid("truncated terminal catalog record"))?;
    *source = &source[count..];
    Ok(value)
}

fn check_counts(arity: usize, count: usize, limits: BinaryIoLimits) -> Result<(), BinaryIoError> {
    check_limit(
        "terminal catalog arity",
        arity,
        limits.max_collection_entries,
    )?;
    check_limit(
        "terminal catalog entries",
        count,
        limits.max_collection_entries,
    )?;
    let coordinates = arity
        .checked_mul(count)
        .ok_or(BinaryIoError::Invalid("terminal coordinate count overflow"))?;
    check_limit(
        "terminal catalog coordinates",
        coordinates,
        limits.max_collection_entries,
    )
}

fn native_error(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}
fn allocation(resource: &'static str, requested: usize) -> BinaryIoError {
    BinaryIoError::Allocation {
        resource,
        requested,
    }
}
