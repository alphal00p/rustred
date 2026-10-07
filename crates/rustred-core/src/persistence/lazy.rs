//! Framing-only index and on-demand use of the existing native atom decoder.
//! Trusted generated input only, exactly like DecodedCoefficientTable.

use super::error::check_limit;
use super::native::{preflight_native_frame, preflight_state_export, read_length};
use super::{BinaryIoError, BinaryIoLimits, CoefficientId};
use crate::algebra::Coefficient;
use std::{
    collections::BTreeMap,
    ops::Range,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex},
};
use symbolica::state::{State, StateMap};

struct Imported {
    state: Option<StateMap>,
    coefficients: BTreeMap<usize, Arc<Coefficient>>,
}

/// Owns encoded bytes and a bounded frame index, not eagerly decoded algebra.
/// State import occurs once on first selected access; only accessed coefficients
/// are cached. IDs remain local to this exact table.
pub struct LazyDecodedCoefficientTable {
    state: Arc<[u8]>,
    atoms: Arc<[u8]>,
    frames: Vec<Range<usize>>,
    limits: BinaryIoLimits,
    imported: Mutex<Imported>,
}

impl LazyDecodedCoefficientTable {
    pub fn new(
        state: Arc<[u8]>,
        atoms: Arc<[u8]>,
        limits: BinaryIoLimits,
    ) -> Result<Self, BinaryIoError> {
        check_limit("Symbolica state bytes", state.len(), limits.max_state_bytes)?;
        check_limit(
            "coefficient table bytes",
            atoms.len(),
            limits.max_total_atom_bytes,
        )?;
        check_limit(
            "coefficient program bytes",
            state
                .len()
                .checked_add(atoms.len())
                .ok_or(BinaryIoError::Invalid("coefficient byte count overflow"))?,
            limits.max_program_bytes,
        )?;
        let mut source = &atoms[..];
        let count = read_length(&mut source)?;
        check_limit(
            "coefficient table entries",
            count,
            limits.max_collection_entries,
        )?;
        if count > 0 {
            CoefficientId::try_from_index(count - 1)?;
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "coefficient frame index",
                requested: count,
            })?;
        for _ in 0..count {
            let length = read_length(&mut source)?;
            check_limit("coefficient atom bytes", length, limits.max_atom_bytes)?;
            let frame = source
                .get(..length)
                .ok_or(BinaryIoError::Invalid("truncated coefficient atom"))?;
            preflight_native_frame(frame)?;
            let start = atoms.len() - source.len();
            frames.push(start..start + length);
            source = &source[length..];
        }
        if !source.is_empty() {
            return Err(BinaryIoError::Invalid("trailing coefficient table bytes"));
        }
        preflight_state_export(&state)?;
        Ok(Self {
            state,
            atoms,
            frames,
            limits,
            imported: Mutex::new(Imported {
                state: None,
                coefficients: BTreeMap::new(),
            }),
        })
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    pub fn decoded_count(&self) -> Result<usize, BinaryIoError> {
        Ok(self
            .imported
            .lock()
            .map_err(|_| BinaryIoError::Native("lazy coefficient lock poisoned".into()))?
            .coefficients
            .len())
    }
    pub fn coefficient(&self, id: CoefficientId) -> Result<Arc<Coefficient>, BinaryIoError> {
        let frame = self
            .frames
            .get(id.index())
            .ok_or(BinaryIoError::Invalid("coefficient ID is outside table"))?;
        let mut imported = self
            .imported
            .lock()
            .map_err(|_| BinaryIoError::Native("lazy coefficient lock poisoned".into()))?;
        if let Some(value) = imported.coefficients.get(&id.index()) {
            return Ok(value.clone());
        }
        if imported.state.is_none() {
            imported.state = Some(
                catch_unwind(AssertUnwindSafe(|| {
                    let mut source = &self.state[..];
                    let map = State::import(&mut source, None)
                        .map_err(|e| BinaryIoError::Native(e.to_string()))?;
                    if !source.is_empty() {
                        return Err(BinaryIoError::Invalid("trailing Symbolica state bytes"));
                    }
                    Ok(map)
                }))
                .map_err(|_| {
                    BinaryIoError::Native("native Symbolica state import panicked".into())
                })??,
            );
        }
        let value = Arc::new(super::atoms::decode_generated_frame(
            &self.atoms[frame.clone()],
            imported.state.as_ref().expect("imported"),
            self.limits,
            false,
        )?);
        imported.coefficients.insert(id.index(), value.clone());
        Ok(value)
    }
}
