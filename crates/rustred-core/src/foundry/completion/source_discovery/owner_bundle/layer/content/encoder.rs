use crate::foundry::completion::stratum::StratumRegistryError;

const LAYER_RESOURCE: &str = "closed-sector layer canonical content bytes";

enum CanonicalOutput {
    Digest(blake3::Hasher),
    Exact(Vec<u8>),
}

/// Self-delimiting canonical encoder with a hard byte envelope.
///
/// Published layer identity and compact owner-order keys stream directly into
/// BLAKE3. Exact bytes are materialized only on the cold collision fallback,
/// where digest equality is never treated as structural equality.
pub(super) struct BoundedContentHasher {
    output: CanonicalOutput,
    bytes: usize,
    limit: usize,
    resource: &'static str,
}

impl BoundedContentHasher {
    pub(super) fn new(limit: usize) -> Self {
        Self::digest(limit, LAYER_RESOURCE)
    }

    pub(super) fn digest(limit: usize, resource: &'static str) -> Self {
        Self {
            output: CanonicalOutput::Digest(blake3::Hasher::new()),
            bytes: 0,
            limit,
            resource,
        }
    }

    pub(super) fn exact(limit: usize, resource: &'static str) -> Self {
        Self {
            output: CanonicalOutput::Exact(Vec::new()),
            bytes: 0,
            limit,
            resource,
        }
    }

    pub(super) fn raw(&mut self, value: &[u8]) -> Result<(), StratumRegistryError> {
        let requested = self.bytes.checked_add(value.len()).ok_or(
            StratumRegistryError::ResourceCountOverflow {
                resource: self.resource,
            },
        )?;
        if requested > self.limit {
            return Err(StratumRegistryError::ResourceLimit {
                resource: self.resource,
                requested,
                limit: self.limit,
            });
        }
        match &mut self.output {
            CanonicalOutput::Digest(hasher) => {
                hasher.update(value);
            }
            CanonicalOutput::Exact(bytes) => {
                bytes.try_reserve_exact(value.len()).map_err(|_| {
                    StratumRegistryError::AllocationFailure {
                        resource: self.resource,
                        requested,
                    }
                })?;
                bytes.extend_from_slice(value);
            }
        }
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
        let value =
            u64::try_from(value).map_err(|_| StratumRegistryError::ResourceCountOverflow {
                resource: self.resource,
            })?;
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
        let CanonicalOutput::Digest(hasher) = self.output else {
            unreachable!("digest output is fixed by the canonical encoder constructor")
        };
        format!(
            "rustred.closed-sector-layer-content.v2:{}:{}",
            hasher.finalize().to_hex(),
            self.bytes
        )
    }

    pub(super) fn finish_digest(self) -> ([u8; blake3::OUT_LEN], usize) {
        let CanonicalOutput::Digest(hasher) = self.output else {
            unreachable!("digest output is fixed by the canonical encoder constructor")
        };
        (*hasher.finalize().as_bytes(), self.bytes)
    }

    pub(super) fn finish_exact(self) -> Box<[u8]> {
        let CanonicalOutput::Exact(bytes) = self.output else {
            unreachable!("exact output is fixed by the canonical encoder constructor")
        };
        bytes.into_boxed_slice()
    }
}
