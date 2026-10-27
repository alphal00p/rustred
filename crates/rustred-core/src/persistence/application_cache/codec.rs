//! Only integer keys/counts/IDs are RustRed framing. All algebra bytes and
//! symbol remapping use the existing generated native coefficient-table codec.
use std::{
    ops::Range,
    sync::{Arc, Mutex},
};

use super::*;
use crate::persistence::bytes::{ByteCursor, SharedCoefficientBytes};
use crate::persistence::error::check_limit;
use crate::persistence::native::LENGTH_BYTES;
use crate::persistence::{
    CoefficientId, LazyDecodedCoefficientTable, resource_allocation, resource_array,
    resource_checkpoint,
};

const MAGIC: &[u8; 16] = b"RR-CACHE-CHNK-01";

fn native(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}
#[derive(Debug)]
struct Row {
    key: ApplicationCacheKey,
    terms: Range<usize>,
    term_count: usize,
    guards: Range<usize>,
    guard_count: usize,
}

struct ChunkLayout {
    rows: Vec<Row>,
    state: Range<usize>,
    atoms: Range<usize>,
    index_owned_bytes: usize,
}

fn inspect_layout(
    mut read: ByteCursor<'_>,
    expected: &ApplicationCacheBinding,
    limits: ApplicationCacheIoLimits,
) -> Result<ChunkLayout, BinaryIoError> {
    let length = read.remaining();
    expected.validate(limits)?;
    check_limit("cache chunk bytes", length, limits.binary.max_program_bytes)?;
    if read.take(MAGIC.len())?.as_ref() != MAGIC
        || read.take(32)?.as_ref() != expected.artifact_digest
    {
        return Err(BinaryIoError::Invalid(
            "cache magic or artifact identity mismatch",
        ));
    }
    if !read.matches_frame(
        expected.family_fingerprint.as_bytes(),
        limits
            .diagnostics_enabled
            .then_some(limits.max_identity_bytes),
    )? || !read.matches_frame(
        expected.layer.as_bytes(),
        limits
            .diagnostics_enabled
            .then_some(limits.max_identity_bytes),
    )? || read.count(
        limits.diagnostics_enabled.then_some(limits.max_arity),
        "cache arity",
    )? != expected.physical_arity
    {
        return Err(BinaryIoError::Invalid(
            "cache family, layer or arity mismatch",
        ));
    }
    let count = read.count(
        limits.diagnostics_enabled.then_some(limits.max_records),
        "cache records",
    )?;
    // Even with diagnostics off, each declared row must fit its minimum
    // on-wire key and two count frames before any row-index allocation.
    let minimum_row_bytes = expected
        .physical_arity
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(1 + 2 * LENGTH_BYTES))
        .ok_or(BinaryIoError::Invalid("cache row framing overflow"))?;
    if count > read.remaining() / minimum_row_bytes {
        return Err(BinaryIoError::Invalid("truncated cache row directory"));
    }
    let index_bytes = count
        .checked_mul(std::mem::size_of::<Row>())
        .ok_or(BinaryIoError::Invalid("cache index size overflow"))?;
    check_limit(
        "cache index bytes",
        index_bytes,
        limits.max_index_owned_bytes,
    )?;
    let mut rows = Vec::<Row>::new();
    resource_array::<Row>(count)?;
    rows.try_reserve_exact(count)
        .map_err(|_| BinaryIoError::Allocation {
            resource: "cache row index",
            requested: count,
        })?;
    let mut index_owned_bytes = rows
        .capacity()
        .checked_mul(std::mem::size_of::<Row>())
        .ok_or(BinaryIoError::Invalid("cache index size overflow"))?;
    check_limit(
        "cache index bytes",
        index_owned_bytes,
        limits.max_index_owned_bytes,
    )?;
    resource_checkpoint()?;
    for _ in 0..count {
        resource_checkpoint()?;
        let key = read.key(expected.physical_arity)?;
        index_owned_bytes = index_owned_bytes
            .checked_add(key.owned_heap_bytes())
            .ok_or(BinaryIoError::Invalid("cache index size overflow"))?;
        check_limit(
            "cache index bytes",
            index_owned_bytes,
            limits.max_index_owned_bytes,
        )?;
        if rows.last().is_some_and(|previous| previous.key >= key) {
            return Err(BinaryIoError::Invalid(
                "cache keys are not strictly ordered",
            ));
        }
        let term_count = read.count(
            limits
                .diagnostics_enabled
                .then_some(limits.max_terms_per_record),
            "cache record terms",
        )?;
        let term_bytes = expected
            .physical_arity
            .checked_mul(8)
            .and_then(|n| n.checked_add(4))
            .and_then(|n| n.checked_mul(term_count))
            .ok_or(BinaryIoError::Invalid("cache term framing overflow"))?;
        let start = read.position();
        read.skip(term_bytes)?;
        let terms = start..start + term_bytes;
        let guard_count = read.count(
            limits
                .diagnostics_enabled
                .then_some(limits.max_guards_per_record),
            "cache record guards",
        )?;
        let guard_bytes = guard_count
            .checked_mul(4)
            .ok_or(BinaryIoError::Invalid("cache guard framing overflow"))?;
        let start = read.position();
        read.skip(guard_bytes)?;
        rows.push(Row {
            key,
            terms,
            term_count,
            guards: start..start + guard_bytes,
            guard_count,
        });
    }
    let state = read.frame_range(Some(limits.binary.max_state_bytes))?;
    let atoms = read.frame_range(Some(limits.binary.max_total_atom_bytes))?;
    if read.remaining() != 0 {
        return Err(BinaryIoError::Invalid("trailing cache chunk bytes"));
    }
    Ok(ChunkLayout {
        rows,
        state,
        atoms,
        index_owned_bytes,
    })
}

/// A bounded chunk-local key index borrowing its encoded payload. Inspection
/// imports no Symbolica state. The caller can discard it after one lookup.
pub struct ApplicationCacheChunk<'a> {
    bytes: &'a [u8],
    rows: Vec<Row>,
    state: &'a [u8],
    atoms: &'a [u8],
    arity: usize,
    limits: ApplicationCacheIoLimits,
}

impl<'a> ApplicationCacheChunk<'a> {
    pub fn inspect(
        bytes: &'a [u8],
        expected: &ApplicationCacheBinding,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        let layout = inspect_layout(ByteCursor::borrowed(bytes), expected, limits)?;
        Ok(Self {
            bytes,
            rows: layout.rows,
            state: &bytes[layout.state],
            atoms: &bytes[layout.atoms],
            arity: expected.physical_arity,
            limits,
        })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn first_key(&self) -> Option<&ApplicationCacheKey> {
        self.rows.first().map(|row| &row.key)
    }
    pub fn last_key(&self) -> Option<&ApplicationCacheKey> {
        self.rows.last().map(|row| &row.key)
    }

    /// Complete sorted exact keys from the validated native framing. This
    /// borrows metadata only and never imports Symbolica state or coefficients.
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &ApplicationCacheKey> + Clone {
        self.rows.iter().map(|row| &row.key)
    }

    /// Native import is a trusted-generated boundary, not a hostile parser.
    /// Only coefficients referenced by the selected record are decoded. The
    /// temporary table and its duplicate payloads are dropped on return.
    pub fn decode_record(
        &self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, BinaryIoError> {
        key.validate(self.arity)?;
        let Ok(index) = self.rows.binary_search_by(|row| row.key.cmp(key)) else {
            return Ok(None);
        };
        let row = &self.rows[index];
        resource_allocation(
            self.state
                .len()
                .checked_add(self.atoms.len())
                .ok_or(BinaryIoError::Invalid("cache table copy size overflow"))?,
        )?;
        let table = LazyDecodedCoefficientTable::new(
            Arc::from(self.state),
            Arc::from(self.atoms),
            self.limits.binary,
        )?;
        decode_row(
            &self.bytes[row.terms.clone()],
            &self.bytes[row.guards.clone()],
            row,
            &table,
            context,
            self.limits,
            None,
        )
        .map(Some)
    }
}

/// A retained, immutable native chunk with one shared coefficient table.
///
/// Inspection validates framing and the exact binding but grants no reduction
/// authority. Coefficients are imported once, lazily, and each selected record
/// is still checked against the caller's current coefficient context and limits.
/// Unlike the borrowed chunk adapter, repeated hits neither copy the native
/// table nor import its state again. Clones should share this object through Arc.
/// The decoded coefficient census also bounds the aggregate unique table, in
/// addition to the existing per-record clone-owned limits. A failed admission
/// leaves previously admitted coefficients usable.
pub struct OwnedApplicationCacheChunk {
    bytes: SharedCoefficientBytes,
    rows: Vec<Row>,
    table: LazyDecodedCoefficientTable,
    decoded_census: Mutex<crate::reduction::CacheWeight>,
    arity: usize,
    limits: ApplicationCacheIoLimits,
    index_owned_bytes: usize,
}
impl OwnedApplicationCacheChunk {
    pub fn inspect(
        bytes: Arc<[u8]>,
        expected: &ApplicationCacheBinding,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        Self::inspect_bytes(SharedCoefficientBytes::from_arc(bytes), expected, limits)
    }

    /// Move an existing decoded Vec into the shared owner without reallocating
    /// its entire payload as an Arc slice.
    pub fn inspect_owned(
        bytes: Vec<u8>,
        expected: &ApplicationCacheBinding,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        Self::inspect_bytes(SharedCoefficientBytes::from_vec(bytes), expected, limits)
    }

    /// Retain a private decoded reader with a bounded block-digest index. One
    /// sequential pass verifies the complete native digest; subsequent selected
    /// ranges verify their blocks again, so truncation or changed backing bytes
    /// fail gracefully. No mapped slices or caller-defined AsRef owner is used.
    pub fn inspect_reader(
        reader: impl std::io::Read + std::io::Seek + Send + 'static,
        length: usize,
        native_digest: &[u8; 32],
        expected: &ApplicationCacheBinding,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        expected.validate(limits)?;
        check_limit("cache chunk bytes", length, limits.binary.max_program_bytes)?;
        let bytes = SharedCoefficientBytes::from_reader(
            reader,
            length,
            native_digest,
            limits.max_index_owned_bytes,
        )?;
        Self::inspect_bytes(bytes, expected, limits)
    }

    fn inspect_bytes(
        bytes: SharedCoefficientBytes,
        expected: &ApplicationCacheBinding,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        let source_index_bytes = bytes.index_owned_bytes()?;
        let mut framing_limits = limits;
        check_limit(
            "native backing digest index bytes",
            source_index_bytes,
            limits.max_index_owned_bytes,
        )?;
        framing_limits.max_index_owned_bytes = limits.max_index_owned_bytes - source_index_bytes;
        let inspected = inspect_layout(ByteCursor::shared(&bytes), expected, framing_limits)?;
        let atoms = bytes.slice(inspected.atoms.clone())?;
        let coefficient_count = ByteCursor::shared(&atoms).length()?;
        let index_owned_bytes = coefficient_count
            .checked_mul(std::mem::size_of::<Range<usize>>())
            .and_then(|n| n.checked_add(inspected.index_owned_bytes))
            .and_then(|n| n.checked_add(source_index_bytes))
            .ok_or(BinaryIoError::Invalid("cache index size overflow"))?;
        check_limit(
            "cache index bytes",
            index_owned_bytes,
            limits.max_index_owned_bytes,
        )?;
        let table = LazyDecodedCoefficientTable::from_shared(
            bytes.slice(inspected.state)?,
            atoms,
            limits.binary,
        )?;
        let index_owned_bytes = table
            .index_owned_bytes()?
            .checked_add(inspected.index_owned_bytes)
            .and_then(|n| n.checked_add(source_index_bytes))
            .ok_or(BinaryIoError::Invalid("cache index size overflow"))?;
        check_limit(
            "cache index bytes",
            index_owned_bytes,
            limits.max_index_owned_bytes,
        )?;
        Ok(Self {
            bytes,
            rows: inspected.rows,
            table,
            decoded_census: Mutex::default(),
            arity: expected.physical_arity,
            limits,
            index_owned_bytes,
        })
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
    pub fn index_owned_bytes(&self) -> usize {
        self.index_owned_bytes
    }
    pub fn keys(&self) -> impl ExactSizeIterator<Item = &ApplicationCacheKey> + Clone {
        self.rows.iter().map(|row| &row.key)
    }
    pub fn decoded_coefficient_count(&self) -> Result<usize, BinaryIoError> {
        self.table.decoded_count()
    }

    pub fn state_import_count(&self) -> Result<usize, BinaryIoError> {
        self.table.state_import_count()
    }
    /// Aggregate unique coefficient census retained by the shared lazy table.
    pub fn decoded_coefficient_census(&self) -> Result<(usize, usize), BinaryIoError> {
        let census = self
            .decoded_census
            .lock()
            .map_err(|_| native("cache census lock poisoned"))?;
        Ok((census.coefficient_terms, census.coefficient_bytes))
    }
    pub fn decode_record(
        &self,
        key: &ApplicationCacheKey,
        context: &CoefficientContext,
    ) -> Result<Option<ApplicationCacheRecord>, BinaryIoError> {
        key.validate(self.arity)?;
        let Ok(index) = self.rows.binary_search_by(|row| row.key.cmp(key)) else {
            return Ok(None);
        };
        let row = &self.rows[index];
        let term_range = self.bytes.slice(row.terms.clone())?;
        let guard_range = self.bytes.slice(row.guards.clone())?;
        let terms = term_range.read()?;
        let guards = guard_range.read()?;
        decode_row(
            terms.as_ref(),
            guards.as_ref(),
            row,
            &self.table,
            context,
            self.limits,
            Some(&self.decoded_census),
        )
        .map(Some)
    }
}

fn decode_row(
    term_bytes: &[u8],
    guard_bytes: &[u8],
    row: &Row,
    table: &LazyDecodedCoefficientTable,
    context: &CoefficientContext,
    limits: ApplicationCacheIoLimits,
    total_census: Option<&Mutex<crate::reduction::CacheWeight>>,
) -> Result<ApplicationCacheRecord, BinaryIoError> {
    let arity = row.key.powers.len();
    let mut census = crate::reduction::CacheWeight::default();
    let mut coefficient = |id: u32| -> Result<Coefficient, BinaryIoError> {
        resource_checkpoint()?;
        let value = table.coefficient_with_admission(
            CoefficientId::try_from_index(id as usize)?,
            |value| {
                validate_coefficient(value, context, limits)?;
                if let Some(total_census) = total_census {
                    let mut total = total_census
                        .lock()
                        .map_err(|_| native("cache census lock poisoned"))?;
                    let prospective = total
                        .checked_add(coefficient_cache_weight([value]).map_err(reduction_error)?)
                        .map_err(reduction_error)?;
                    limits.check_count(
                        "shared decoded cache coefficient terms",
                        prospective.coefficient_terms,
                        limits.max_decoded_coefficient_terms,
                    )?;
                    check_limit(
                        "shared decoded cache coefficient bytes",
                        prospective.coefficient_bytes,
                        limits.max_decoded_coefficient_bytes,
                    )?;
                    *total = prospective;
                }
                Ok(())
            },
        )?;
        validate_coefficient(&value, context, limits)?;
        let weight = coefficient_cache_weight([value.as_ref()]).map_err(reduction_error)?;
        census = census.checked_add(weight).map_err(reduction_error)?;
        limits.check_count(
            "decoded cache coefficient terms",
            census.coefficient_terms,
            limits.max_decoded_coefficient_terms,
        )?;
        check_limit(
            "decoded cache coefficient bytes",
            census.coefficient_bytes,
            limits.max_decoded_coefficient_bytes,
        )?;
        resource_allocation(weight.coefficient_bytes)?;
        let value = (*value).clone();
        resource_checkpoint()?;
        Ok(value)
    };
    let mut terms = BTreeMap::new();
    let mut read = ByteCursor::borrowed(term_bytes);
    for _ in 0..row.term_count {
        resource_checkpoint()?;
        let target = IntegralKey::try_from_preallocated(read.powers(arity)?).map_err(native)?;
        let value = coefficient(read.id()?)?;
        if value.is_zero()
            || terms
                .last_key_value()
                .is_some_and(|(last, _)| last >= &target)
        {
            return Err(BinaryIoError::Invalid(
                "unordered, duplicate or zero cache terminal",
            ));
        }
        resource_array::<(IntegralKey, Coefficient)>(1)?;
        terms.insert(target, value);
        resource_checkpoint()?;
    }
    let mut nonzero_conditions = Vec::new();
    resource_array::<Coefficient>(row.guard_count)?;
    nonzero_conditions
        .try_reserve_exact(row.guard_count)
        .map_err(|_| BinaryIoError::Allocation {
            resource: "cache guards",
            requested: row.guard_count,
        })?;
    let mut read = ByteCursor::borrowed(guard_bytes);
    for _ in 0..row.guard_count {
        resource_checkpoint()?;
        let value = coefficient(read.id()?)?;
        if value.is_zero() {
            return Err(BinaryIoError::Invalid("vanished cache guard"));
        }
        nonzero_conditions.push(value);
    }
    resource_allocation(row.key.owned_heap_bytes())?;
    let key = row.key.clone();
    resource_checkpoint()?;
    Ok(ApplicationCacheRecord {
        key,
        terms,
        nonzero_conditions,
    })
}

impl ByteCursor<'_> {
    fn count(
        &mut self,
        limit: Option<usize>,
        resource: &'static str,
    ) -> Result<usize, BinaryIoError> {
        resource_checkpoint()?;
        let count = self.length()?;
        if let Some(limit) = limit {
            check_limit(resource, count, limit)?;
        }
        Ok(count)
    }
    fn matches_frame(
        &mut self,
        expected: &[u8],
        limit: Option<usize>,
    ) -> Result<bool, BinaryIoError> {
        let count = self.count(limit, "cache frame bytes")?;
        if count != expected.len() {
            return Ok(false);
        }
        Ok(self.take(count)?.as_ref() == expected)
    }
    fn frame_range(&mut self, limit: Option<usize>) -> Result<Range<usize>, BinaryIoError> {
        let count = self.count(limit, "cache frame bytes")?;
        self.skip(count)
    }
    fn powers(&mut self, arity: usize) -> Result<Vec<i64>, BinaryIoError> {
        let bytes = arity
            .checked_mul(std::mem::size_of::<i64>())
            .ok_or(BinaryIoError::Invalid("cache powers framing overflow"))?;
        let encoded = self.take(bytes)?;
        let mut powers = Vec::new();
        resource_array::<i64>(arity)?;
        powers
            .try_reserve_exact(arity)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "cache powers",
                requested: arity,
            })?;
        for value in encoded.as_chunks::<8>().0 {
            resource_checkpoint()?;
            powers.push(i64::from_le_bytes(*value));
        }
        Ok(powers)
    }
    fn key(&mut self, arity: usize) -> Result<ApplicationCacheKey, BinaryIoError> {
        let stage = match self.take(1)?[0] {
            0 => ApplicationCacheStage::GuardedRoot,
            1 => ApplicationCacheStage::Route,
            2 => ApplicationCacheStage::Apply,
            _ => return Err(BinaryIoError::Invalid("unknown cache node stage")),
        };
        let owner = if stage == ApplicationCacheStage::Apply {
            let encoded = self.take(arity)?;
            resource_array::<bool>(arity)?;
            let mut owner = Vec::new();
            owner
                .try_reserve_exact(arity)
                .map_err(|_| BinaryIoError::Allocation {
                    resource: "cache owner",
                    requested: arity,
                })?;
            for &value in encoded.iter() {
                resource_checkpoint()?;
                owner.push(match value {
                    0 => false,
                    1 => true,
                    _ => return Err(BinaryIoError::Invalid("nonboolean cache owner")),
                });
            }
            owner
        } else {
            Vec::new()
        };
        let key = ApplicationCacheKey {
            stage,
            owner,
            powers: self.powers(arity)?,
        };
        key.validate(arity)?;
        Ok(key)
    }
    fn id(&mut self) -> Result<u32, BinaryIoError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.as_ref().try_into().expect("checked slice"),
        ))
    }
}

#[cfg(test)]
mod reader_admission_tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom};

    struct NoIo;
    impl Read for NoIo {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            panic!("admission must precede file I/O")
        }
    }
    impl Seek for NoIo {
        fn seek(&mut self, _: SeekFrom) -> std::io::Result<u64> {
            panic!("admission must precede file I/O")
        }
    }

    #[test]
    fn reader_byte_and_binding_grants_are_checked_before_scanning() {
        let mut binding = ApplicationCacheBinding {
            artifact_digest: [0; 32],
            family_fingerprint: "reader-admission".into(),
            physical_arity: 2,
            layer: "native".into(),
        };
        let mut limits = ApplicationCacheIoLimits::default();
        limits.binary.max_program_bytes = 0;
        assert!(matches!(
            OwnedApplicationCacheChunk::inspect_reader(NoIo, 1, &[0; 32], &binding, limits),
            Err(BinaryIoError::Limit { .. })
        ));
        binding.family_fingerprint.clear();
        assert!(matches!(
            OwnedApplicationCacheChunk::inspect_reader(NoIo, 0, &[0; 32], &binding, limits),
            Err(BinaryIoError::Invalid(_))
        ));
    }
}
