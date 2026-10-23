use std::io::{self, Write};

use crate::algebra::ExactAlgebraLimits;

/// Caller-owned transport budgets. They do not assert that Symbolica's native
/// decoder validates every malformed internal allocation count.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BinaryIoLimits {
    pub max_program_bytes: usize,
    pub max_sections: usize,
    pub max_collection_entries: usize,
    pub max_state_bytes: usize,
    pub max_atom_bytes: usize,
    pub max_total_atom_bytes: usize,
    pub exact_algebra: ExactAlgebraLimits,
}

impl BinaryIoLimits {
    /// Explicit trusted-native transport policy without byte/count quotas.
    /// Native representations, checked size arithmetic and allocation failures
    /// still apply. This does not turn generated Symbolica records into a
    /// hostile-input sandbox; the caller owns process/RAM admission.
    pub const fn unbounded_resources() -> Self {
        Self {
            max_program_bytes: usize::MAX,
            max_sections: usize::MAX,
            max_collection_entries: usize::MAX,
            max_state_bytes: usize::MAX,
            max_atom_bytes: usize::MAX,
            max_total_atom_bytes: usize::MAX,
            exact_algebra: ExactAlgebraLimits::unbounded_resources(),
        }
    }
}

impl Default for BinaryIoLimits {
    fn default() -> Self {
        Self {
            max_program_bytes: 1024 * 1024 * 1024,
            max_sections: 32,
            max_collection_entries: 8_000_000,
            max_state_bytes: 16 * 1024 * 1024,
            max_atom_bytes: 16 * 1024 * 1024,
            max_total_atom_bytes: 512 * 1024 * 1024,
            exact_algebra: ExactAlgebraLimits::default(),
        }
    }
}

/// Bound native export output before growing its destination buffer.
pub(super) struct CappedWriter {
    bytes: Vec<u8>,
    resource: &'static str,
    limit: usize,
}

impl CappedWriter {
    pub(super) fn new(resource: &'static str, limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            resource,
            limit,
        }
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let requested = self.bytes.len().checked_add(bytes.len()).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "native output byte count overflow",
            )
        })?;
        if requested > self.limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{} requires {requested} bytes; limit is {}",
                    self.resource, self.limit
                ),
            ));
        }
        self.bytes.try_reserve(bytes.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::OutOfMemory,
                "native output allocation failed",
            )
        })?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::persistence::{
        BinaryIoError, BinaryProgramKind, BinarySection, SectionTag, encode_program,
        inspect_program,
    };

    #[test]
    fn unbounded_native_transport_has_only_machine_representation_grants() {
        let io = BinaryIoLimits::unbounded_resources();
        for grant in [
            io.max_program_bytes,
            io.max_sections,
            io.max_collection_entries,
            io.max_state_bytes,
            io.max_atom_bytes,
            io.max_total_atom_bytes,
        ] {
            assert_eq!(grant, usize::MAX);
        }
        assert_eq!(io.exact_algebra, ExactAlgebraLimits::unbounded_resources());
    }

    #[test]
    fn explicit_default_and_smaller_native_transport_grants_still_apply() {
        let io = BinaryIoLimits::default();
        let bytes = encode_program(
            BinaryProgramKind::Candidates,
            &[BinarySection {
                tag: SectionTag::PROGRAM,
                bytes: &[1, 2, 3],
            }],
            io,
        )
        .unwrap();
        inspect_program(&bytes, io).unwrap();
        let small = BinaryIoLimits {
            max_program_bytes: bytes.len() - 1,
            ..io
        };
        assert!(matches!(
            inspect_program(&bytes, small),
            Err(BinaryIoError::Limit {
                resource: "program bytes",
                ..
            })
        ));
        inspect_program(&bytes, BinaryIoLimits::unbounded_resources()).unwrap();
    }
}
