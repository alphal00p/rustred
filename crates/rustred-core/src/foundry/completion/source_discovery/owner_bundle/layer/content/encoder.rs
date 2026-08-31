use crate::foundry::completion::stratum::StratumRegistryError;

const RESOURCE: &str = "closed-sector layer canonical content bytes";

/// Streaming, allocation-light canonical encoder with a hard byte envelope.
///
/// Length prefixes and fixed-width big-endian integers make the stream
/// unambiguous. Only the final BLAKE3 digest and processed-byte count are
/// retained; the potentially large canonical payload is never materialized.
pub(super) struct BoundedContentHasher {
    hasher: blake3::Hasher,
    bytes: usize,
    limit: usize,
}

impl BoundedContentHasher {
    pub(super) fn new(limit: usize) -> Self {
        Self {
            hasher: blake3::Hasher::new(),
            bytes: 0,
            limit,
        }
    }

    pub(super) fn raw(&mut self, value: &[u8]) -> Result<(), StratumRegistryError> {
        let requested = self
            .bytes
            .checked_add(value.len())
            .ok_or(StratumRegistryError::ResourceCountOverflow { resource: RESOURCE })?;
        if requested > self.limit {
            return Err(StratumRegistryError::ResourceLimit {
                resource: RESOURCE,
                requested,
                limit: self.limit,
            });
        }
        self.hasher.update(value);
        self.bytes = requested;
        Ok(())
    }

    pub(super) fn tag(&mut self, tag: u8) -> Result<(), StratumRegistryError> {
        self.raw(&[tag])
    }

    pub(super) fn boolean(&mut self, value: bool) -> Result<(), StratumRegistryError> {
        self.tag(u8::from(value))
    }

    pub(super) fn u16(&mut self, value: u16) -> Result<(), StratumRegistryError> {
        self.raw(&value.to_be_bytes())
    }

    pub(super) fn u32(&mut self, value: u32) -> Result<(), StratumRegistryError> {
        self.raw(&value.to_be_bytes())
    }

    pub(super) fn u64(&mut self, value: u64) -> Result<(), StratumRegistryError> {
        self.raw(&value.to_be_bytes())
    }

    pub(super) fn usize(&mut self, value: usize) -> Result<(), StratumRegistryError> {
        let value = u64::try_from(value)
            .map_err(|_| StratumRegistryError::ResourceCountOverflow { resource: RESOURCE })?;
        self.u64(value)
    }

    pub(super) fn i64(&mut self, value: i64) -> Result<(), StratumRegistryError> {
        self.raw(&value.to_be_bytes())
    }

    pub(super) fn i128(&mut self, value: i128) -> Result<(), StratumRegistryError> {
        self.raw(&value.to_be_bytes())
    }

    pub(super) fn text(&mut self, value: &str) -> Result<(), StratumRegistryError> {
        self.usize(value.len())?;
        self.raw(value.as_bytes())
    }

    pub(super) fn count(&mut self, value: usize) -> Result<(), StratumRegistryError> {
        self.usize(value)
    }

    pub(super) fn finish(self) -> String {
        format!(
            "rustred.closed-sector-layer-content.v1:{}:{}",
            self.hasher.finalize().to_hex(),
            self.bytes
        )
    }
}
