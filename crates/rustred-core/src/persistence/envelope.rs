use std::io::Write;

use super::error::check_limit;
use super::limits::CappedWriter;
use super::{BinaryIoError, BinaryIoLimits};

const MAGIC: &[u8; 8] = b"RRPBIN\r\n";
pub const BINARY_PROGRAM_VERSION: u32 = 1;
const HEADER_BYTES: usize = 20;
const SECTION_HEADER_BYTES: usize = 12;

/// A payload claim, not a proof or a constructor for trusted rule ownership.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BinaryProgramKind {
    Candidates = 1,
    Certified = 2,
    /// Exact externally prepared values; no reduction or closure authority.
    TerminalValues = 3,
    /// Finite exact output normalization, never an IBP closure certificate.
    TerminalNormalization = 4,
    /// Source-replayed closure on an explicit total-excess entry domain.
    /// Distinct from an unrestricted Certified promise.
    BoundedCertified = 5,
}

/// Structural sections shared by independently admitted program payloads.
/// Unknown tags are retained by inspection; semantic loaders reject unsupported
/// required sections instead of silently interpreting them as trusted data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct SectionTag(pub u16);

impl SectionTag {
    pub const SYMBOLICA_STATE: Self = Self(1);
    pub const COEFFICIENTS: Self = Self(2);
    pub const FAMILY: Self = Self(3);
    pub const PROGRAM: Self = Self(4);
    pub const PROVENANCE: Self = Self(5);
    pub const CERTIFICATE: Self = Self(6);
    pub const VALUES: Self = Self(7);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinarySection<'a> {
    pub tag: SectionTag,
    pub bytes: &'a [u8],
}

#[derive(Debug)]
pub struct ProgramEnvelope<'a> {
    kind: BinaryProgramKind,
    sections: Vec<BinarySection<'a>>,
}

impl<'a> ProgramEnvelope<'a> {
    pub fn kind(&self) -> BinaryProgramKind {
        self.kind
    }

    pub fn sections(&self) -> &[BinarySection<'a>] {
        &self.sections
    }

    pub fn section(&self, tag: SectionTag) -> Option<&'a [u8]> {
        self.sections
            .binary_search_by_key(&tag, |section| section.tag)
            .ok()
            .map(|index| self.sections[index].bytes)
    }
}

/// Write only RustRed's structural framing. Symbolica owns the algebra bytes.
/// Sections must have strictly increasing tags; this catches duplicate owners.
pub fn encode_program(
    kind: BinaryProgramKind,
    sections: &[BinarySection<'_>],
    limits: BinaryIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    require_native_word_size(std::mem::size_of::<usize>())?;
    check_limit("section count", sections.len(), limits.max_sections)?;
    let count = u32::try_from(sections.len())
        .map_err(|_| BinaryIoError::Invalid("section count exceeds wire width"))?;
    let mut total = HEADER_BYTES;
    let mut previous = None;
    for section in sections {
        check_tag_order(previous, section.tag)?;
        previous = Some(section.tag);
        check_section(section, limits)?;
        total = total
            .checked_add(SECTION_HEADER_BYTES)
            .and_then(|size| size.checked_add(section.bytes.len()))
            .ok_or(BinaryIoError::Invalid("program length overflow"))?;
    }
    check_limit("program bytes", total, limits.max_program_bytes)?;
    let mut out = CappedWriter::new("program bytes", limits.max_program_bytes);
    let write = |result: std::io::Result<()>| {
        result.map_err(|error| BinaryIoError::Native(error.to_string()))
    };
    write(out.write_all(MAGIC))?;
    write(out.write_all(&BINARY_PROGRAM_VERSION.to_le_bytes()))?;
    write(out.write_all(&[kind as u8, 8, 0, 0]))?;
    write(out.write_all(&count.to_le_bytes()))?;
    for section in sections {
        write(out.write_all(&section.tag.0.to_le_bytes()))?;
        write(out.write_all(&0u16.to_le_bytes()))?;
        let len = u64::try_from(section.bytes.len())
            .map_err(|_| BinaryIoError::Invalid("section length exceeds wire width"))?;
        write(out.write_all(&len.to_le_bytes()))?;
        write(out.write_all(section.bytes))?;
    }
    Ok(out.finish())
}

/// Inspect bounded borrowed sections without invoking any native decoder.
/// This operation is safe for arbitrary bytes; it grants no algebraic authority.
pub fn inspect_program(
    bytes: &[u8],
    limits: BinaryIoLimits,
) -> Result<ProgramEnvelope<'_>, BinaryIoError> {
    check_limit("program bytes", bytes.len(), limits.max_program_bytes)?;
    let mut cursor = Cursor { bytes, offset: 0 };
    if cursor.take(8)? != MAGIC {
        return Err(BinaryIoError::Invalid(
            "magic does not identify a binary program",
        ));
    }
    if cursor.u32()? != BINARY_PROGRAM_VERSION {
        return Err(BinaryIoError::Invalid("unsupported binary program version"));
    }
    let kind = match cursor.take(1)?[0] {
        1 => BinaryProgramKind::Candidates,
        2 => BinaryProgramKind::Certified,
        3 => BinaryProgramKind::TerminalValues,
        4 => BinaryProgramKind::TerminalNormalization,
        5 => BinaryProgramKind::BoundedCertified,
        _ => return Err(BinaryIoError::Invalid("unsupported binary program kind")),
    };
    require_native_word_size(usize::from(cursor.take(1)?[0]))?;
    require_native_word_size(std::mem::size_of::<usize>())?;
    if cursor.u16()? != 0 {
        return Err(BinaryIoError::Invalid("nonzero reserved header flags"));
    }
    let count = usize::try_from(cursor.u32()?)
        .map_err(|_| BinaryIoError::Invalid("section count exceeds host width"))?;
    check_limit("section count", count, limits.max_sections)?;
    if count > cursor.remaining() / SECTION_HEADER_BYTES {
        return Err(BinaryIoError::Invalid(
            "section count exceeds remaining framing",
        ));
    }
    let mut sections = Vec::new();
    sections
        .try_reserve_exact(count)
        .map_err(|_| BinaryIoError::Allocation {
            resource: "section entries",
            requested: count,
        })?;
    let mut previous = None;
    for _ in 0..count {
        let tag = SectionTag(cursor.u16()?);
        check_tag_order(previous, tag)?;
        previous = Some(tag);
        if cursor.u16()? != 0 {
            return Err(BinaryIoError::Invalid("nonzero reserved section flags"));
        }
        let length = usize::try_from(cursor.u64()?)
            .map_err(|_| BinaryIoError::Invalid("section length exceeds host width"))?;
        let section = BinarySection {
            tag,
            bytes: cursor.take(length)?,
        };
        check_section(&section, limits)?;
        sections.push(section);
    }
    if cursor.remaining() != 0 {
        return Err(BinaryIoError::Invalid(
            "trailing bytes after program sections",
        ));
    }
    Ok(ProgramEnvelope { kind, sections })
}

fn check_tag_order(previous: Option<SectionTag>, tag: SectionTag) -> Result<(), BinaryIoError> {
    if tag.0 == 0 || previous.is_some_and(|previous| previous >= tag) {
        Err(BinaryIoError::Invalid(
            "section tags must be nonzero and strictly increasing",
        ))
    } else {
        Ok(())
    }
}

fn require_native_word_size(bytes: usize) -> Result<(), BinaryIoError> {
    if bytes == 8 {
        Ok(())
    } else {
        Err(BinaryIoError::Invalid(
            "native Symbolica Atom format currently requires a 64-bit host",
        ))
    }
}

fn check_section(section: &BinarySection<'_>, limits: BinaryIoLimits) -> Result<(), BinaryIoError> {
    let limit = if section.tag == SectionTag::SYMBOLICA_STATE {
        limits.max_state_bytes
    } else {
        limits.max_program_bytes
    };
    check_limit("section bytes", section.bytes.len(), limit)
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn remaining(&self) -> usize {
        self.bytes.len() - self.offset
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], BinaryIoError> {
        let end = self
            .offset
            .checked_add(count)
            .ok_or(BinaryIoError::Invalid("section offset overflow"))?;
        let result = self
            .bytes
            .get(self.offset..end)
            .ok_or(BinaryIoError::Invalid("truncated binary program"))?;
        self.offset = end;
        Ok(result)
    }

    fn u16(&mut self) -> Result<u16, BinaryIoError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("exact width"),
        ))
    }

    fn u32(&mut self) -> Result<u32, BinaryIoError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("exact width"),
        ))
    }

    fn u64(&mut self) -> Result<u64, BinaryIoError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("exact width"),
        ))
    }
}

#[cfg(test)]
#[path = "envelope/tests.rs"]
mod tests;
