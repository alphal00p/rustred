//! Structural indices around the existing shared native Symbolica codec.
use super::*;
use rustred::family::IntegralFamilyLimits;
use rustred::persistence::{
    CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable, NativeFamilyRecord,
};
use std::collections::HashMap;

const MAGIC: &[u8; 16] = b"RR-APPLY-MAPS-01";

#[derive(bincode::Encode, bincode::Decode)]
struct Record {
    families: Vec<(String, NativeFamilyRecord)>,
    rows: Vec<(Key, Vec<(Key, u32)>, Vec<u32>)>,
    remaining: Vec<Key>,
    state: Vec<u8>,
    atoms: Vec<u8>,
}
#[derive(bincode::Encode, bincode::Decode)]
struct Key {
    family: u32,
    powers: Vec<i64>,
}

pub(super) fn encode(
    maps: &TerminalMaps,
    options: SavedApplicationOptions,
) -> Result<Vec<u8>, AppError> {
    maps.validate(options)?;
    let mut table = CoefficientTableBuilder::new(options.binary_limits);
    let families = maps
        .families
        .iter()
        .map(|(fingerprint, family)| {
            Ok((
                fingerprint.clone(),
                NativeFamilyRecord::from_family(family, &mut table).map_err(native_failure)?,
            ))
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let family_ids = families
        .iter()
        .enumerate()
        .map(|(i, (f, _))| (f.as_str(), i as u32))
        .collect::<BTreeMap<_, _>>();
    let key = |key: &SavedApplicationIntegral| Key {
        family: family_ids[key.family_fingerprint.as_str()],
        powers: key.integral.powers().to_vec(),
    };
    let mut rows = Vec::new();
    let mut guard_ids = HashMap::<*const Vec<Coefficient>, Vec<u32>>::new();
    for (source, row) in &maps.rows {
        let terms = row
            .terms
            .iter()
            .map(|(k, c)| {
                Ok((
                    key(k),
                    table.intern(c).map_err(native_failure)?.index() as u32,
                ))
            })
            .collect::<Result<_, AppError>>()?;
        let pointer = Arc::as_ptr(&row.conditions);
        let conditions = if let Some(ids) = guard_ids.get(&pointer) {
            ids.clone()
        } else {
            let ids = row
                .conditions
                .iter()
                .map(|c| {
                    table
                        .intern(c)
                        .map(|id| id.index() as u32)
                        .map_err(native_failure)
                })
                .collect::<Result<Vec<_>, _>>()?;
            guard_ids.insert(pointer, ids.clone());
            ids
        };
        rows.push((key(source), terms, conditions));
    }
    let remaining = maps.remaining.iter().map(key).collect();
    let encoded = table.finish().map_err(native_failure)?;
    let record = Record {
        families,
        rows,
        remaining,
        state: encoded.state,
        atoms: encoded.atoms,
    };
    let mut bytes = MAGIC.to_vec();
    bincode::encode_into_std_write(record, &mut bytes, bincode::config::standard())
        .map_err(native_failure)?;
    if bytes.len() > options.binary_limits.max_program_bytes {
        return Err(AppError::limit(
            "compact terminal maps exceed binary allowance",
        ));
    }
    Ok(bytes)
}

pub(super) fn decode(
    bytes: &[u8],
    options: SavedApplicationOptions,
) -> Result<TerminalMaps, AppError> {
    if bytes.len() > options.binary_limits.max_program_bytes || !bytes.starts_with(MAGIC) {
        return Err(AppError::input(
            "invalid compact terminal map framing or byte allowance",
        ));
    }
    // Existing native ingress is trusted generated data. Bound outer structural
    // allocations before delegating atom/state validation to its native codec.
    let (record, consumed): (Record, usize) = super::super::candidate_bundle::decode_record(
        &bytes[MAGIC.len()..],
        options.binary_limits.max_program_bytes,
    )?;
    if consumed != bytes.len() - MAGIC.len() {
        return Err(AppError::input("trailing bytes in compact terminal maps"));
    }
    if options.binary_limits.diagnostics_enabled
        && (record.rows.len() > options.binary_limits.max_collection_entries
            || record.remaining.len() > options.binary_limits.max_collection_entries
            || record.families.len() > 1024)
    {
        return Err(AppError::limit(
            "compact terminal map collection allowance exceeded",
        ));
    }
    let table = DecodedCoefficientTable::import_generated(
        &record.state,
        &record.atoms,
        options.binary_limits,
    )
    .map_err(native_failure)?;
    let mut families = BTreeMap::new();
    let mut fingerprints = Vec::new();
    for (fingerprint, encoded) in record.families {
        let family = encoded
            .to_family(
                &table,
                IntegralFamilyLimits::default(),
                options.binary_limits,
            )
            .map_err(native_failure)?;
        if family.fingerprint() != fingerprint
            || families
                .insert(fingerprint.clone(), Arc::new(family))
                .is_some()
        {
            return Err(AppError::input(
                "compact terminal family fingerprint mismatch or duplicate",
            ));
        }
        fingerprints.push(fingerprint);
    }
    let key = |key: Key| -> Result<SavedApplicationIntegral, AppError> {
        Ok(SavedApplicationIntegral {
            family_fingerprint: fingerprints
                .get(key.family as usize)
                .ok_or_else(|| AppError::input("invalid compact terminal family index"))?
                .clone(),
            integral: IntegralKey::try_new(key.powers).map_err(native_failure)?,
        })
    };
    let coefficient = |id: u32| -> Result<Coefficient, AppError> {
        Ok(table
            .coefficient(CoefficientId::try_from_index(id as usize).map_err(native_failure)?)
            .map_err(native_failure)?
            .clone())
    };
    let mut remaining = BTreeSet::new();
    for output in record.remaining {
        if !remaining.insert(key(output)?) {
            return Err(AppError::input("duplicate compact remaining terminal"));
        }
    }
    let mut rows = BTreeMap::new();
    let mut guard_vectors = HashMap::<Vec<u32>, Arc<Vec<Coefficient>>>::new();
    for (source, terms, conditions) in record.rows {
        let mut output = BTreeMap::new();
        for (target, id) in terms {
            if output.insert(key(target)?, coefficient(id)?).is_some() {
                return Err(AppError::input(
                    "duplicate compact terminal substitution term",
                ));
            }
        }
        // The wire format remains unchanged. Identical ordered native table
        // references share their immutable decoded coefficients across rows.
        let conditions = if let Some(shared) = guard_vectors.get(&conditions) {
            shared.clone()
        } else {
            let shared = Arc::new(
                conditions
                    .iter()
                    .copied()
                    .map(coefficient)
                    .collect::<Result<Vec<_>, _>>()?,
            );
            guard_vectors.insert(conditions, shared.clone());
            shared
        };
        if rows
            .insert(
                key(source)?,
                TerminalRow {
                    terms: output,
                    conditions,
                },
            )
            .is_some()
        {
            return Err(AppError::input("duplicate compact terminal source"));
        }
    }
    let maps = TerminalMaps {
        families,
        rows,
        remaining,
    };
    maps.validate(options)?;
    Ok(maps)
}

/// Check every source row, including every retained guard, once while exporting
/// from the authenticated publication. Runtime loading needs no search matrix.
pub(super) fn verify_roundtrip(
    maps: &TerminalMaps,
    bytes: &[u8],
    options: SavedApplicationOptions,
) -> Result<(), AppError> {
    let restored = decode(bytes, options)?;
    if maps.rows != restored.rows
        || maps.remaining != restored.remaining
        || maps.families.keys().ne(restored.families.keys())
    {
        return Err(AppError::execution(
            "compact terminal export differs from source substitutions",
        ));
    }
    Ok(())
}
