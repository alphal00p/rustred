//! Shared framing for Symbolica-owned Atom bytes; no algebra codec lives here.

use std::io::Write;

use symbolica::state::{HasStateMap, StateMap};

use super::BinaryIoError;

pub(super) const LENGTH_BYTES: usize = 8;
pub(super) const NATIVE_ATOM_HEADER_BYTES: usize = 9;

pub(super) struct BorrowedStateMap<'a>(pub(super) &'a StateMap);

impl HasStateMap for BorrowedStateMap<'_> {
    fn get_state_map(&self) -> &StateMap {
        self.0
    }
}

pub(super) fn ensure_native_word_size() -> Result<(), BinaryIoError> {
    // The pinned native encoder writes usize, while its decoder reads u64.
    if std::mem::size_of::<usize>() != LENGTH_BYTES {
        return Err(BinaryIoError::Invalid(
            "native atom bincode requires a 64-bit target",
        ));
    }
    Ok(())
}

pub(super) fn write_length(writer: &mut impl Write, value: usize) -> Result<(), BinaryIoError> {
    let value = u64::try_from(value).map_err(|_| BinaryIoError::Invalid("length exceeds u64"))?;
    writer
        .write_all(&value.to_le_bytes())
        .map_err(|error| BinaryIoError::Native(error.to_string()))
}

pub(super) fn read_length(source: &mut &[u8]) -> Result<usize, BinaryIoError> {
    let bytes = source
        .get(..LENGTH_BYTES)
        .ok_or(BinaryIoError::Invalid("truncated length"))?;
    let value = u64::from_le_bytes(bytes.try_into().expect("checked length"));
    *source = &source[LENGTH_BYTES..];
    usize::try_from(value).map_err(|_| BinaryIoError::Invalid("length exceeds usize"))
}

pub(super) fn preflight_native_frame(frame: &[u8]) -> Result<(), BinaryIoError> {
    if frame.len() <= NATIVE_ATOM_HEADER_BYTES || frame[0] != 0 {
        return Err(BinaryIoError::Invalid("invalid native atom frame"));
    }
    let mut length_source = &frame[1..NATIVE_ATOM_HEADER_BYTES];
    let length = read_length(&mut length_source)?;
    if length != frame.len() - NATIVE_ATOM_HEADER_BYTES {
        return Err(BinaryIoError::Invalid(
            "native atom length differs from frame",
        ));
    }
    Ok(())
}
