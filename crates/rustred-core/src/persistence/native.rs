//! Shared framing for Symbolica-owned Atom bytes; no algebra codec lives here.

use std::io::Write;
use std::sync::OnceLock;

use symbolica::atom::Atom;
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

// Symbolica's Atom encoder writes one format byte and a fixed u64 length on
// every target. RustRed uses the same fixed framing, independently of usize.
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

/// Rejection message for artifacts written against Symbolica export format 5
/// (RustRed before the patch-free Symbolica pin); their headers can be rewritten
/// by the converter named here.
pub(super) const LEGACY_EXPORT_FORMAT: &str = "native artifact uses Symbolica export format 5 \
     (written by RustRed before the Symbolica ef0db494 pin); convert it with \
     examples/python/convert_native_v5_to_v6.py";

/// Symbolica export (state) format version that the pre-ef0db494 RustRed wrote.
const LEGACY_STATE_EXPORT_VERSION: u16 = 5;
/// Atom storage-format byte of that legacy export.
const LEGACY_ATOM_FORMAT: u8 = 0;
/// Magic that starts a Symbolica state export.
const STATE_EXPORT_MAGIC: u32 = 0x3787_1367;

/// The atom storage-format byte written by the linked Symbolica (0 up to v3.0.0,
/// 1 since upstream main a19c760d). Derived once from Symbolica itself so the
/// preflight tracks the pinned revision instead of hard-coding its layout.
pub(super) fn native_atom_format() -> u8 {
    static FORMAT: OnceLock<u8> = OnceLock::new();
    *FORMAT.get_or_init(|| {
        let bytes = bincode::encode_to_vec(Atom::num(1), bincode::config::standard())
            .expect("encoding a small integer atom cannot fail");
        bytes[0]
    })
}

/// Refuses a Symbolica state export in the legacy format 5 with a message that
/// names the converter; any other version is left to Symbolica's own import.
pub(super) fn preflight_state_export(state: &[u8]) -> Result<(), BinaryIoError> {
    if state.len() >= 6
        && u32::from_le_bytes(state[..4].try_into().expect("checked length")) == STATE_EXPORT_MAGIC
        && u16::from_le_bytes(state[4..6].try_into().expect("checked length"))
            == LEGACY_STATE_EXPORT_VERSION
    {
        return Err(BinaryIoError::Invalid(LEGACY_EXPORT_FORMAT));
    }
    Ok(())
}

pub(super) fn preflight_native_frame(frame: &[u8]) -> Result<(), BinaryIoError> {
    if frame.len() <= NATIVE_ATOM_HEADER_BYTES {
        return Err(BinaryIoError::Invalid("invalid native atom frame"));
    }
    let format = native_atom_format();
    if frame[0] != format {
        if frame[0] == LEGACY_ATOM_FORMAT && format != LEGACY_ATOM_FORMAT {
            return Err(BinaryIoError::Invalid(LEGACY_EXPORT_FORMAT));
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(format: u8) -> Vec<u8> {
        let mut frame = vec![format];
        frame.extend_from_slice(&1u64.to_le_bytes());
        frame.push(1);
        frame
    }

    #[test]
    fn lengths_are_always_fixed_u64_not_host_pointer_width() {
        let mut bytes = Vec::new();
        write_length(&mut bytes, 513).unwrap();
        assert_eq!(bytes, 513u64.to_le_bytes());
        assert_eq!(read_length(&mut bytes.as_slice()).unwrap(), 513);
    }

    #[cfg(target_pointer_width = "32")]
    #[test]
    fn oversized_native_lengths_are_rejected_before_atom_allocation() {
        let length = (u64::from(u32::MAX) + 1).to_le_bytes();
        assert!(read_length(&mut length.as_slice()).is_err());
        let mut bytes = frame(native_atom_format());
        bytes[1..9].copy_from_slice(&length);
        assert!(preflight_native_frame(&bytes).is_err());
    }

    #[test]
    fn preflight_accepts_the_linked_atom_format_and_names_the_converter_for_format_five() {
        let format = native_atom_format();
        let bytes =
            bincode::encode_to_vec(Atom::num(7), bincode::config::standard()).expect("encode");
        assert_eq!(bytes[0], format);
        preflight_native_frame(&frame(format)).expect("linked format accepted");
        if format != LEGACY_ATOM_FORMAT {
            let error = preflight_native_frame(&frame(LEGACY_ATOM_FORMAT)).unwrap_err();
            assert!(error.to_string().contains("convert_native_v5_to_v6.py"));
        }
        let error = preflight_native_frame(&frame(0xff)).unwrap_err();
        assert!(error.to_string().contains("invalid native atom frame"));
    }

    #[test]
    fn preflight_rejects_a_format_five_state_export_with_the_converter_name() {
        let mut state = STATE_EXPORT_MAGIC.to_le_bytes().to_vec();
        state.extend_from_slice(&LEGACY_STATE_EXPORT_VERSION.to_le_bytes());
        state.push(0);
        let error = preflight_state_export(&state).unwrap_err();
        assert!(error.to_string().contains("convert_native_v5_to_v6.py"));
        state[4] = 6;
        preflight_state_export(&state).expect("current export left to Symbolica");
    }
}
