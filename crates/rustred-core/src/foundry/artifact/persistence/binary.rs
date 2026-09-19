use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use crate::algebra::Coefficient;
use crate::persistence::{
    CoefficientId, CoefficientTableBuilder, DecodedCoefficientTable, EncodedCoefficientTable,
    same_native_coefficient,
};

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

fn coefficient_hash(value: &Coefficient) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.numerator.variables().hash(&mut hasher);
    value.denominator.variables().hash(&mut hasher);
    value.hash(&mut hasher);
    hasher.finish()
}

/// Source/rule replay uses input IDs only after matching the complete native
/// value, including its variable map. Hash collisions never establish equality.
struct NativeLookup {
    table: Option<Rc<DecodedCoefficientTable>>,
    buckets: HashMap<u64, Vec<CoefficientId>>,
}

impl NativeLookup {
    fn new(table: Rc<DecodedCoefficientTable>) -> Result<Self, ArtifactPersistenceError> {
        let mut buckets: HashMap<u64, Vec<CoefficientId>> = HashMap::new();
        buckets.try_reserve(table.len()).map_err(|_| {
            ArtifactPersistenceError::AllocationFailure {
                resource: "native coefficient lookup",
                requested: table.len(),
            }
        })?;
        for index in 0..table.len() {
            let id = CoefficientId::try_from_index(index)?;
            let value = table.coefficient(id)?;
            let bucket = buckets.entry(coefficient_hash(value)).or_default();
            for previous in bucket.iter() {
                if same_native_coefficient(table.coefficient(*previous)?, value) {
                    return Err(ArtifactPersistenceError::SemanticMismatch {
                        field: "duplicate native coefficient table entry",
                    });
                }
            }
            bucket
                .try_reserve(1)
                .map_err(|_| ArtifactPersistenceError::AllocationFailure {
                    resource: "native coefficient collision bucket",
                    requested: bucket.len().saturating_add(1),
                })?;
            bucket.push(id);
        }
        Ok(Self {
            table: Some(table),
            buckets,
        })
    }

    fn coefficient(&self, id: CoefficientId) -> Result<&Coefficient, ArtifactPersistenceError> {
        self.table
            .as_ref()
            .ok_or(ArtifactPersistenceError::InvalidCoefficient {
                field: "missing native coefficient table",
            })?
            .coefficient(id)
            .map_err(Into::into)
    }

    fn find(&self, value: &Coefficient) -> Result<CoefficientId, ArtifactPersistenceError> {
        if let Some(bucket) = self.buckets.get(&coefficient_hash(value)) {
            for id in bucket {
                if same_native_coefficient(self.coefficient(*id)?, value) {
                    return Ok(*id);
                }
            }
        }
        Err(ArtifactPersistenceError::SemanticMismatch {
            field: "replayed coefficient absent from native table",
        })
    }
}

#[derive(Clone)]
enum NativeWriter {
    Fresh(Rc<RefCell<Option<CoefficientTableBuilder>>>),
    Replay(Rc<NativeLookup>),
}

/// Child sinks share one first-occurrence native coefficient table and one
/// monotonic witness budget. Repeated references incur no repeated atom charge.
pub(super) struct Writer {
    bytes: Vec<u8>,
    limits: ArtifactEncodingLimits,
    native: NativeWriter,
    witness_budget: Rc<ByteBudget>,
}

impl Writer {
    pub(super) fn new(limits: ArtifactEncodingLimits) -> Self {
        Self {
            bytes: Vec::new(),
            limits,
            native: NativeWriter::Fresh(Rc::new(RefCell::new(Some(CoefficientTableBuilder::new(
                limits.native_io(),
            ))))),
            witness_budget: Rc::new(ByteBudget::default()),
        }
    }

    pub(super) fn child(&self) -> Self {
        Self {
            bytes: Vec::new(),
            limits: self.limits,
            native: self.native.clone(),
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

    pub(super) fn intern_coefficient(
        &self,
        value: &Coefficient,
    ) -> Result<CoefficientId, ArtifactPersistenceError> {
        match &self.native {
            NativeWriter::Fresh(builder) => builder
                .borrow_mut()
                .as_mut()
                .ok_or(ArtifactPersistenceError::SemanticMismatch {
                    field: "finished native coefficient interner",
                })?
                .intern(value)
                .map_err(Into::into),
            NativeWriter::Replay(lookup) => lookup.find(value),
        }
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

    pub(super) fn finish_native(
        self,
    ) -> Result<(Vec<u8>, EncodedCoefficientTable), ArtifactPersistenceError> {
        let NativeWriter::Fresh(builder) = self.native else {
            return Err(ArtifactPersistenceError::SemanticMismatch {
                field: "native table requested from replay writer",
            });
        };
        let builder =
            builder
                .borrow_mut()
                .take()
                .ok_or(ArtifactPersistenceError::SemanticMismatch {
                    field: "finished native coefficient interner",
                })?;
        Ok((self.bytes, builder.finish()?))
    }

    #[cfg(test)]
    pub(super) fn finish_for_test(
        self,
    ) -> Result<(Vec<u8>, Rc<DecodedCoefficientTable>), ArtifactPersistenceError> {
        let limits = self.limits.native_io();
        let (bytes, encoded) = self.finish_native()?;
        let table =
            DecodedCoefficientTable::import_generated(&encoded.state, &encoded.atoms, limits)?;
        Ok((bytes, Rc::new(table)))
    }

    #[cfg(test)]
    pub(super) fn seeded_for_test(
        table: &DecodedCoefficientTable,
    ) -> Result<Self, ArtifactPersistenceError> {
        let writer = Self::new(Default::default());
        for index in 0..table.len() {
            let id = CoefficientId::try_from_index(index)?;
            if writer.intern_coefficient(table.coefficient(id)?)? != id {
                return Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "nonunique test coefficient table",
                });
            }
        }
        Ok(writer)
    }
}

/// One bounded immutable byte cursor. Every child cursor shares the same
/// native coefficient lookup and witness budgets.
pub(super) struct Reader<'input> {
    input: &'input [u8],
    offset: usize,
    limits: ArtifactLoadLimits,
    native: Rc<NativeLookup>,
    witness_budget: Rc<ByteBudget>,
}

impl<'input> Reader<'input> {
    #[cfg(test)]
    pub(super) fn root(
        input: &'input [u8],
        limits: ArtifactLoadLimits,
    ) -> Result<Self, ArtifactPersistenceError> {
        check_limit("artifact bytes", input.len(), limits.max_artifact_bytes)?;
        Ok(Self {
            input,
            offset: 0,
            limits,
            native: Rc::new(NativeLookup {
                table: None,
                buckets: HashMap::new(),
            }),
            witness_budget: Rc::new(ByteBudget::default()),
        })
    }

    pub(super) fn with_table(
        input: &'input [u8],
        limits: ArtifactLoadLimits,
        table: Rc<DecodedCoefficientTable>,
    ) -> Result<Self, ArtifactPersistenceError> {
        check_limit("artifact bytes", input.len(), limits.max_artifact_bytes)?;
        Ok(Self {
            input,
            offset: 0,
            limits,
            native: Rc::new(NativeLookup::new(table)?),
            witness_budget: Rc::new(ByteBudget::default()),
        })
    }

    pub(super) fn native_coefficient(
        &self,
        id: CoefficientId,
    ) -> Result<&Coefficient, ArtifactPersistenceError> {
        self.native.coefficient(id)
    }

    pub(super) fn child(&self, input: &'input [u8]) -> Self {
        Self {
            input,
            offset: 0,
            limits: self.limits,
            native: self.native.clone(),
            witness_budget: self.witness_budget.clone(),
        }
    }

    /// Local replay resolves regenerated values to input IDs by exact native
    /// equality. Final document replay uses a separate, fresh interner instead.
    pub(super) fn replay_writer(&self) -> Writer {
        Writer {
            bytes: Vec::new(),
            limits: self.limits.replay_encoding(),
            native: NativeWriter::Replay(self.native.clone()),
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

#[cfg(test)]
mod native_lookup_tests {
    use super::*;
    use crate::algebra::CoefficientContext;

    #[test]
    fn constants_keep_same_width_maps_even_in_forced_hash_collisions() {
        let base = CoefficientContext::try_new(["lookup_native_d", "lookup_native_s"]).unwrap();
        let reversed = CoefficientContext::try_new(["lookup_native_s", "lookup_native_d"]).unwrap();
        let foreign = CoefficientContext::try_new(["lookup_native_d", "lookup_native_q"]).unwrap();
        for number in [0, 1] {
            let first = base.integer(number);
            let second = reversed.integer(number);
            let missing = foreign.integer(number);
            // Symbolica intentionally equates these algebraic constants. The
            // artifact's reference identity must nevertheless retain its map.
            assert_eq!(first, second);
            let mut builder = CoefficientTableBuilder::new(Default::default());
            let first_id = builder.intern(&first).unwrap();
            let second_id = builder.intern(&second).unwrap();
            assert_ne!(first_id, second_id);
            let encoded = builder.finish().unwrap();
            let table = Rc::new(
                DecodedCoefficientTable::import_generated(
                    &encoded.state,
                    &encoded.atoms,
                    Default::default(),
                )
                .unwrap(),
            );
            let mut lookup = NativeLookup::new(table).unwrap();
            assert_eq!(lookup.find(&first).unwrap(), first_id);
            assert_eq!(lookup.find(&second).unwrap(), second_id);
            lookup
                .buckets
                .insert(coefficient_hash(&second), vec![first_id, second_id]);
            assert_eq!(lookup.find(&second).unwrap(), second_id);
            lookup
                .buckets
                .insert(coefficient_hash(&missing), vec![first_id, second_id]);
            assert!(matches!(
                lookup.find(&missing),
                Err(ArtifactPersistenceError::SemanticMismatch {
                    field: "replayed coefficient absent from native table"
                })
            ));
            let different_value = base.integer(number + 7);
            lookup.buckets.insert(
                coefficient_hash(&different_value),
                vec![first_id, second_id],
            );
            assert!(lookup.find(&different_value).is_err());
        }
    }
}
