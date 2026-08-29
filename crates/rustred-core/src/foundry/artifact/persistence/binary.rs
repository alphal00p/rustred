use std::cell::Cell;
use std::rc::Rc;

use super::super::error::ArtifactPersistenceError;
use super::limits::{ArtifactEncodingLimits, ArtifactLoadLimits};

#[derive(Default)]
struct ByteBudget {
    used: Cell<usize>,
}

impl ByteBudget {
    fn charge(
        &self,
        additional: usize,
        limit: usize,
        resource: &'static str,
    ) -> Result<(), ArtifactPersistenceError> {
        let requested = self
            .used
            .get()
            .checked_add(additional)
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow { resource })?;
        check_limit(resource, requested, limit)?;
        self.used.set(requested);
        Ok(())
    }
}

/// One bounded deterministic byte sink. Child sinks own independent output
/// buffers but share the same monotonic coefficient and witness budgets.
pub(super) struct Writer {
    bytes: Vec<u8>,
    limits: ArtifactEncodingLimits,
    coefficient_budget: Rc<ByteBudget>,
    witness_budget: Rc<ByteBudget>,
}

impl Writer {
    pub(super) fn new(limits: ArtifactEncodingLimits) -> Self {
        Self {
            bytes: Vec::new(),
            limits,
            coefficient_budget: Rc::new(ByteBudget::default()),
            witness_budget: Rc::new(ByteBudget::default()),
        }
    }

    pub(super) fn child(&self) -> Self {
        Self {
            bytes: Vec::new(),
            limits: self.limits,
            coefficient_budget: self.coefficient_budget.clone(),
            witness_budget: self.witness_budget.clone(),
        }
    }

    fn reserve(
        &mut self,
        additional: usize,
        resource: &'static str,
    ) -> Result<(), ArtifactPersistenceError> {
        let requested = self
            .bytes
            .len()
            .checked_add(additional)
            .ok_or(ArtifactPersistenceError::ResourceCountOverflow { resource })?;
        check_limit(
            "encoded artifact bytes",
            requested,
            self.limits.max_artifact_bytes,
        )?;
        self.bytes.try_reserve_exact(additional).map_err(|_| {
            ArtifactPersistenceError::AllocationFailure {
                resource,
                requested,
            }
        })
    }

    pub(super) fn u8(&mut self, value: u8) -> Result<(), ArtifactPersistenceError> {
        self.reserve(1, "encoded bytes")?;
        self.bytes.push(value);
        Ok(())
    }

    pub(super) fn u16(&mut self, value: u16) -> Result<(), ArtifactPersistenceError> {
        self.raw(&value.to_le_bytes())
    }

    pub(super) fn u32(&mut self, value: u32) -> Result<(), ArtifactPersistenceError> {
        self.raw(&value.to_le_bytes())
    }

    pub(super) fn u64(&mut self, value: u64) -> Result<(), ArtifactPersistenceError> {
        self.raw(&value.to_le_bytes())
    }

    pub(super) fn i64(&mut self, value: i64) -> Result<(), ArtifactPersistenceError> {
        self.raw(&value.to_le_bytes())
    }

    pub(super) fn usize(
        &mut self,
        value: usize,
        resource: &'static str,
    ) -> Result<(), ArtifactPersistenceError> {
        check_limit(resource, value, self.limits.max_collection_entries)?;
        let value = u64::try_from(value)
            .map_err(|_| ArtifactPersistenceError::ResourceCountOverflow { resource })?;
        self.u64(value)
    }

    pub(super) fn raw(&mut self, value: &[u8]) -> Result<(), ArtifactPersistenceError> {
        self.reserve(value.len(), "encoded bytes")?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    pub(super) fn bytes(
        &mut self,
        value: &[u8],
        resource: &'static str,
    ) -> Result<(), ArtifactPersistenceError> {
        let len = u64::try_from(value.len())
            .map_err(|_| ArtifactPersistenceError::ResourceCountOverflow { resource })?;
        self.u64(len)?;
        self.raw(value)
    }

    pub(super) fn string(
        &mut self,
        value: &str,
        resource: &'static str,
    ) -> Result<(), ArtifactPersistenceError> {
        check_limit(resource, value.len(), self.limits.max_string_bytes)?;
        self.bytes(value.as_bytes(), resource)
    }

    /// Charge one complete sparse coefficient/polynomial payload before its
    /// child buffer is allocated or written.
    pub(super) fn charge_coefficient_payload(
        &self,
        bytes: usize,
    ) -> Result<(), ArtifactPersistenceError> {
        check_limit(
            "coefficient payload bytes",
            bytes,
            self.limits.max_coefficient_bytes,
        )?;
        self.coefficient_budget.charge(
            bytes,
            self.limits.max_total_coefficient_bytes,
            "aggregate coefficient bytes",
        )
    }

    pub(super) fn charge_witness_payload(
        &self,
        bytes: usize,
    ) -> Result<(), ArtifactPersistenceError> {
        self.witness_budget.charge(
            bytes,
            self.limits.max_total_witness_bytes,
            "aggregate semantic witness bytes",
        )
    }

    pub(super) fn limits(&self) -> ArtifactEncodingLimits {
        self.limits
    }

    pub(super) fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

/// One bounded immutable byte cursor. Every child cursor shares the same
/// coefficient and witness budgets.
pub(super) struct Reader<'input> {
    input: &'input [u8],
    offset: usize,
    limits: ArtifactLoadLimits,
    coefficient_budget: Rc<ByteBudget>,
    witness_budget: Rc<ByteBudget>,
}

impl<'input> Reader<'input> {
    pub(super) fn root(
        input: &'input [u8],
        limits: ArtifactLoadLimits,
    ) -> Result<Self, ArtifactPersistenceError> {
        check_limit("artifact bytes", input.len(), limits.max_artifact_bytes)?;
        Ok(Self {
            input,
            offset: 0,
            limits,
            coefficient_budget: Rc::new(ByteBudget::default()),
            witness_budget: Rc::new(ByteBudget::default()),
        })
    }

    pub(super) fn child(&self, input: &'input [u8]) -> Self {
        Self {
            input,
            offset: 0,
            limits: self.limits,
            coefficient_budget: self.coefficient_budget.clone(),
            witness_budget: self.witness_budget.clone(),
        }
    }

    /// Construct the deterministic semantic-replay sink inside this load's
    /// coefficient budget. Opaque persisted witnesses keep their independent
    /// reader-side budget; only regenerated sparse coefficient payloads join
    /// the family coefficients already admitted by this reader tree.
    pub(super) fn replay_writer(&self) -> Writer {
        Writer {
            bytes: Vec::new(),
            limits: self.limits.replay_encoding(),
            coefficient_budget: self.coefficient_budget.clone(),
            witness_budget: Rc::new(ByteBudget::default()),
        }
    }

    pub(super) fn limits(&self) -> ArtifactLoadLimits {
        self.limits
    }

    fn take(&mut self, len: usize) -> Result<&'input [u8], ArtifactPersistenceError> {
        let end = self.offset.checked_add(len).ok_or(
            ArtifactPersistenceError::ResourceCountOverflow {
                resource: "decoded byte offset",
            },
        )?;
        let value =
            self.input
                .get(self.offset..end)
                .ok_or(ArtifactPersistenceError::Truncated {
                    offset: self.offset,
                })?;
        self.offset = end;
        Ok(value)
    }

    pub(super) fn fixed(&mut self, len: usize) -> Result<&'input [u8], ArtifactPersistenceError> {
        self.take(len)
    }

    pub(super) fn u8(&mut self) -> Result<u8, ArtifactPersistenceError> {
        Ok(self.take(1)?[0])
    }

    pub(super) fn u16(&mut self) -> Result<u16, ArtifactPersistenceError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("fixed length"),
        ))
    }

    pub(super) fn u32(&mut self) -> Result<u32, ArtifactPersistenceError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("fixed length"),
        ))
    }

    pub(super) fn u64(&mut self) -> Result<u64, ArtifactPersistenceError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("fixed length"),
        ))
    }

    pub(super) fn i64(&mut self) -> Result<i64, ArtifactPersistenceError> {
        Ok(i64::from_le_bytes(
            self.take(8)?.try_into().expect("fixed length"),
        ))
    }

    pub(super) fn usize(
        &mut self,
        resource: &'static str,
        limit: usize,
    ) -> Result<usize, ArtifactPersistenceError> {
        let value = usize::try_from(self.u64()?)
            .map_err(|_| ArtifactPersistenceError::ResourceCountOverflow { resource })?;
        check_limit(resource, value, limit)?;
        Ok(value)
    }

    pub(super) fn count(
        &mut self,
        resource: &'static str,
    ) -> Result<usize, ArtifactPersistenceError> {
        self.usize(resource, self.limits.max_collection_entries)
    }

    pub(super) fn bytes(
        &mut self,
        resource: &'static str,
        limit: usize,
    ) -> Result<&'input [u8], ArtifactPersistenceError> {
        let len = self.usize(resource, limit)?;
        self.take(len)
    }

    pub(super) fn string(
        &mut self,
        field: &'static str,
    ) -> Result<&'input str, ArtifactPersistenceError> {
        let bytes = self.bytes(field, self.limits.max_string_bytes)?;
        std::str::from_utf8(bytes).map_err(|_| ArtifactPersistenceError::InvalidUtf8 { field })
    }

    /// Read and globally charge one length-delimited sparse coefficient or
    /// polynomial payload before a decoder allocates native values.
    pub(super) fn coefficient_payload(
        &mut self,
        resource: &'static str,
    ) -> Result<&'input [u8], ArtifactPersistenceError> {
        let bytes = self.bytes(resource, self.limits.max_coefficient_bytes)?;
        self.charge_coefficient_payload(bytes.len())?;
        Ok(bytes)
    }

    /// Charge a decoded native coefficient payload against the artifact-wide
    /// budget shared by every child reader.
    pub(super) fn charge_coefficient_payload(
        &self,
        bytes: usize,
    ) -> Result<(), ArtifactPersistenceError> {
        self.coefficient_budget.charge(
            bytes,
            self.limits.max_total_coefficient_bytes,
            "aggregate coefficient bytes",
        )
    }

    pub(super) fn charge_witness_payload(
        &self,
        bytes: usize,
    ) -> Result<(), ArtifactPersistenceError> {
        self.witness_budget.charge(
            bytes,
            self.limits.max_total_witness_bytes,
            "aggregate semantic witness bytes",
        )
    }

    pub(super) fn section(
        &mut self,
        expected_tag: u16,
    ) -> Result<&'input [u8], ArtifactPersistenceError> {
        let actual = self.u16()?;
        if actual != expected_tag {
            return Err(ArtifactPersistenceError::InvalidSection {
                expected: expected_tag,
                actual,
            });
        }
        self.bytes("section bytes", self.limits.max_artifact_bytes)
    }

    pub(super) fn finish(&self) -> Result<(), ArtifactPersistenceError> {
        let remaining = self.input.len() - self.offset;
        if remaining == 0 {
            Ok(())
        } else {
            Err(ArtifactPersistenceError::TrailingBytes { remaining })
        }
    }
}

pub(super) fn check_limit(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), ArtifactPersistenceError> {
    if requested > limit {
        Err(ArtifactPersistenceError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

pub(super) fn try_vec<T>(
    len: usize,
    resource: &'static str,
) -> Result<Vec<T>, ArtifactPersistenceError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(len)
        .map_err(|_| ArtifactPersistenceError::AllocationFailure {
            resource,
            requested: len,
        })?;
    Ok(result)
}
