//! Framing-only index and on-demand use of the existing native atom decoder.
//! Trusted generated input only, exactly like DecodedCoefficientTable.

use super::bytes::{ByteCursor, SharedCoefficientBytes};
use super::error::check_limit;
use super::native::{preflight_native_frame_header, preflight_state_export};
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
    state: SharedCoefficientBytes,
    atoms: SharedCoefficientBytes,
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
        Self::from_shared(
            SharedCoefficientBytes::from_arc(state),
            SharedCoefficientBytes::from_arc(atoms),
            limits,
        )
    }

    pub(crate) fn from_shared(
        state: SharedCoefficientBytes,
        atoms: SharedCoefficientBytes,
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
        let mut source = ByteCursor::shared(&atoms);
        let count = source.length()?;
        limits.check_count(
            "coefficient table entries",
            count,
            limits.max_collection_entries,
        )?;
        if count > 0 {
            CoefficientId::try_from_index(count - 1)?;
        }
        if count
            > source.remaining()
                / (super::native::LENGTH_BYTES + super::native::NATIVE_ATOM_HEADER_BYTES + 1)
        {
            return Err(BinaryIoError::Invalid(
                "coefficient count exceeds remaining framing",
            ));
        }
        let mut frames = Vec::new();
        super::resource_array::<Range<usize>>(count)?;
        frames
            .try_reserve_exact(count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "coefficient frame index",
                requested: count,
            })?;
        for _ in 0..count {
            let length = source.length()?;
            check_limit("coefficient atom bytes", length, limits.max_atom_bytes)?;
            let start = source.position();
            if length <= super::native::NATIVE_ATOM_HEADER_BYTES {
                return Err(BinaryIoError::Invalid("invalid native atom frame"));
            }
            let header = source.take(super::native::NATIVE_ATOM_HEADER_BYTES)?;
            preflight_native_frame_header(header.as_ref(), length)?;
            source.skip(length - super::native::NATIVE_ATOM_HEADER_BYTES)?;
            frames.push(start..start + length);
        }
        if source.remaining() != 0 {
            return Err(BinaryIoError::Invalid("trailing coefficient table bytes"));
        }
        {
            let header_range = state.slice(0..state.len().min(6))?;
            let header = header_range.read()?;
            preflight_state_export(header.as_ref())?;
        }

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
    pub(crate) fn index_owned_bytes(&self) -> Result<usize, BinaryIoError> {
        self.frames
            .capacity()
            .checked_mul(std::mem::size_of::<Range<usize>>())
            .ok_or(BinaryIoError::Invalid(
                "coefficient frame index size overflow",
            ))
    }
    pub fn decoded_count(&self) -> Result<usize, BinaryIoError> {
        Ok(self
            .imported
            .lock()
            .map_err(|_| BinaryIoError::Native("lazy coefficient lock poisoned".into()))?
            .coefficients
            .len())
    }

    /// Successful native state imports retained by this shared table (zero or
    /// one). Failed imports do not install a state map.
    pub fn state_import_count(&self) -> Result<usize, BinaryIoError> {
        Ok(usize::from(
            self.imported
                .lock()
                .map_err(|_| BinaryIoError::Native("lazy coefficient lock poisoned".into()))?
                .state
                .is_some(),
        ))
    }
    pub fn coefficient(&self, id: CoefficientId) -> Result<Arc<Coefficient>, BinaryIoError> {
        self.coefficient_with_admission(id, |_| Ok(()))
    }

    /// Check a newly decoded coefficient before retaining it. The callback runs
    /// under the table lock exactly once per successful first admission. Existing
    /// entries bypass it; callers must keep one fixed policy for a shared table.
    pub(crate) fn coefficient_with_admission(
        &self,
        id: CoefficientId,
        admit: impl FnOnce(&Coefficient) -> Result<(), BinaryIoError>,
    ) -> Result<Arc<Coefficient>, BinaryIoError> {
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
            super::resource_allocation(self.state.len())?;
            imported.state = Some(
                catch_unwind(AssertUnwindSafe(|| {
                    let state = self.state.read()?;
                    let mut source = super::structure::ResourceReader::new(state.as_ref());
                    let map = State::import(&mut source, None).map_err(super::native_io_error)?;
                    if source.remaining() != 0 {
                        return Err(BinaryIoError::Invalid("trailing Symbolica state bytes"));
                    }
                    Ok(map)
                }))
                .map_err(|_| {
                    BinaryIoError::Native("native Symbolica state import panicked".into())
                })??,
            );
        }
        let frame_range = self.atoms.slice(frame.clone())?;
        let frame = frame_range.read()?;
        let value = Arc::new(super::atoms::decode_generated_frame(
            frame.as_ref(),
            imported.state.as_ref().expect("imported"),
            self.limits,
            false,
        )?);
        admit(&value)?;
        super::resource_array::<(usize, Arc<Coefficient>)>(1)?;
        imported.coefficients.insert(id.index(), value.clone());
        Ok(value)
    }
}
