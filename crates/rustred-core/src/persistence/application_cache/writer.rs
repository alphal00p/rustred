//! Ordered native cache encoding with one shared coefficient table.
//!
//! Records and serialized tables are streamed. The exact coefficient interner
//! still retains every unique compact atom until finalization; this API does
//! not promise constant memory or bound Symbolica/allocator scratch usage.
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::ops::Range;

use super::*;
use crate::persistence::error::check_limit;
use crate::persistence::native::write_length;
use crate::persistence::{
    CoefficientTableBuilder, native_io_error, resource_allocation, resource_checkpoint,
};

const MAGIC: &[u8; 16] = b"RR-CACHE-CHNK-01";

/// Successful output census. Interner figures describe the end of record
/// ingestion, before its indexes and atoms are released during finalization.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ApplicationCacheChunkWriteStatistics {
    pub records: usize,
    /// Successful input rows, including byte-identical unordered duplicates.
    pub input_records: usize,
    pub identical_duplicates: usize,
    /// Temporary unordered row-spool bytes; zero for the ordered API.
    pub unordered_spool_bytes: u64,
    /// Conservative key/range admission census, not an allocator RSS bound.
    pub unordered_index_owned_bytes: usize,
    pub native_bytes: u64,
    pub unique_coefficients: usize,
    /// Unique compact atom payload; excludes table/frame headers.
    pub unique_coefficient_payload_bytes: usize,
    /// Complete shared atom-section size, including counts and frame lengths.
    pub encoded_coefficient_table_bytes: usize,
    /// Known container storage only; excludes allocator/hash control overhead,
    /// shared variable-map payloads, state exports and native scratch.
    pub interner_index_lower_bound_bytes: usize,
}

/// Incremental writer for the existing RR-CACHE-CHNK-01 format.
///
/// Each pushed record must be strictly greater than its predecessor. The writer
/// validates the same native contents as the buffered API, preserves guard order
/// and multiplicity, and holds no record list. It retains the complete unique
/// coefficient interner and one previous key. `finish` appends a single table
/// and patches the record count, restoring the destination's final position.
///
/// The destination may start at a nonzero offset. It must exclusively belong
/// to this writer for its lifetime. Any failed push poisons finalization. Errors
/// may leave partial bytes: durable/atomic publication belongs to the caller.
/// No binding or transport check establishes a mathematical reduction identity.
pub struct ApplicationCacheChunkWriter<'a, W: Write + Seek> {
    output: LimitedOutput<W>,
    binding: ApplicationCacheBinding,
    context: &'a CoefficientContext,
    limits: ApplicationCacheIoLimits,
    table: CoefficientTableBuilder,
    previous: Option<ApplicationCacheKey>,
    records: usize,
    count_offset: u64,
    poisoned: bool,
    unordered: Option<UnorderedRows>,
}

impl<'a, W: Write + Seek> ApplicationCacheChunkWriter<'a, W> {
    pub fn new(
        writer: W,
        binding: &ApplicationCacheBinding,
        context: &'a CoefficientContext,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        resource_checkpoint()?;
        binding.validate(limits)?;
        resource_allocation(
            binding
                .family_fingerprint
                .len()
                .checked_add(binding.layer.len())
                .ok_or(BinaryIoError::Invalid("cache binding byte count overflow"))?,
        )?;
        let mut output = LimitedOutput::new(writer, limits.binary.max_program_bytes)?;
        put(&mut output, MAGIC)?;
        put(&mut output, &binding.artifact_digest)?;
        frame(&mut output, binding.family_fingerprint.as_bytes())?;
        frame(&mut output, binding.layer.as_bytes())?;
        write_length(&mut output, binding.physical_arity)?;
        let count_offset = output.position()?;
        write_length(&mut output, 0)?;
        Ok(Self {
            output,
            binding: binding.clone(),
            context,
            limits,
            table: CoefficientTableBuilder::new(limits.binary),
            previous: None,
            records: 0,
            count_offset,
            poisoned: false,
            unordered: None,
        })
    }

    /// Successfully appended rows; failed pushes are never counted.
    pub fn records_written(&self) -> usize {
        self.records
    }

    /// Native bytes appended so far, excluding the destination's initial prefix.
    /// The state/table footer is included only after finalization.
    pub fn native_bytes_written(&self) -> usize {
        self.output.bytes
    }

    /// Number of unique coefficient atoms currently retained. This and the
    /// payload getter are constant-time monitoring observations, not RAM bounds.
    pub fn unique_coefficients(&self) -> usize {
        self.table.len()
    }

    pub fn unique_coefficient_payload_bytes(&self) -> usize {
        self.table.retained_atom_payload_bytes()
    }

    /// Append one completed native record. No reference to its algebra is kept.
    pub fn push(&mut self, record: &ApplicationCacheRecord) -> Result<(), BinaryIoError> {
        if self.poisoned {
            return Err(BinaryIoError::Invalid("cache chunk writer is poisoned"));
        }
        self.poisoned = true;
        self.push_inner(record)?;
        self.poisoned = false;
        Ok(())
    }

    fn validate_record(
        &self,
        record: &ApplicationCacheRecord,
        count: usize,
    ) -> Result<(), BinaryIoError> {
        resource_checkpoint()?;
        record.key.validate(self.binding.physical_arity)?;
        self.limits
            .check_count("cache records", count, self.limits.max_records)?;
        self.limits.check_count(
            "cache record terms",
            record.terms.len(),
            self.limits.max_terms_per_record,
        )?;
        self.limits.check_count(
            "cache record guards",
            record.nonzero_conditions.len(),
            self.limits.max_guards_per_record,
        )?;
        let (terms, bytes) = record.coefficient_census()?;
        self.limits.check_count(
            "decoded cache coefficient terms",
            terms,
            self.limits.max_decoded_coefficient_terms,
        )?;
        check_limit(
            "decoded cache coefficient bytes",
            bytes,
            self.limits.max_decoded_coefficient_bytes,
        )?;
        for (target, coefficient) in &record.terms {
            resource_checkpoint()?;
            if target.powers().len() != self.binding.physical_arity || coefficient.is_zero() {
                return Err(BinaryIoError::Invalid(
                    "invalid or zero cache terminal term",
                ));
            }
            validate_coefficient(coefficient, self.context, self.limits)?;
        }
        for condition in &record.nonzero_conditions {
            resource_checkpoint()?;
            validate_coefficient(condition, self.context, self.limits)?;
            if condition.is_zero() {
                return Err(BinaryIoError::Invalid("vanished cache guard"));
            }
        }
        Ok(())
    }

    fn push_inner(&mut self, record: &ApplicationCacheRecord) -> Result<(), BinaryIoError> {
        if self.unordered.is_some() {
            return Err(BinaryIoError::Invalid(
                "unordered cache writer requires push_unordered",
            ));
        }
        if self.previous.as_ref().is_some_and(|key| key >= &record.key) {
            return Err(BinaryIoError::Invalid(
                "cache keys are not strictly ordered",
            ));
        }
        let count = self
            .records
            .checked_add(1)
            .ok_or(BinaryIoError::Invalid("cache record count overflow"))?;
        self.validate_record(record, count)?;
        resource_allocation(record.key.owned_heap_bytes())?;
        let previous = record.key.clone();
        encode_row(&mut self.output, &mut self.table, record)?;
        self.previous = Some(previous);
        self.records = count;
        Ok(())
    }

    /// Append state and the one shared coefficient table, then patch the count.
    /// The returned destination is positioned immediately after the native chunk.
    pub fn finish(mut self) -> Result<(W, ApplicationCacheChunkWriteStatistics), BinaryIoError> {
        resource_checkpoint()?;
        if self.poisoned {
            return Err(BinaryIoError::Invalid("cache chunk writer is poisoned"));
        }
        if self.unordered.is_some() {
            return Err(BinaryIoError::Invalid(
                "unordered cache writer requires finish_sorted_into",
            ));
        }
        let mut statistics = ApplicationCacheChunkWriteStatistics {
            records: self.records,
            input_records: self.records,
            unique_coefficients: self.table.len(),
            unique_coefficient_payload_bytes: self.table.retained_atom_payload_bytes(),
            encoded_coefficient_table_bytes: self.table.encoded_atom_bytes(),
            interner_index_lower_bound_bytes: self.table.retained_index_lower_bound_bytes()?,
            ..Default::default()
        };
        self.table.finish_into(&mut self.output)?;
        let end = self.output.position()?;
        resource_checkpoint()?;
        self.output
            .inner
            .seek(SeekFrom::Start(self.count_offset))
            .map_err(native_io_error)?;
        write_length(&mut self.output.inner, self.records)?;
        resource_checkpoint()?;
        self.output
            .inner
            .seek(SeekFrom::Start(end))
            .map_err(native_io_error)?;
        statistics.native_bytes = u64::try_from(self.output.bytes)
            .map_err(|_| BinaryIoError::Invalid("cache chunk length exceeds u64"))?;
        Ok((self.output.inner, statistics))
    }
}

/// Only exact keys and locations are retained; no record algebra is indexed.
struct UnorderedRows {
    rows: BTreeMap<ApplicationCacheKey, Range<u64>>,
    index_bytes: usize,
    identical_duplicates: usize,
}

impl<'a, W: Read + Write + Seek> ApplicationCacheChunkWriter<'a, W> {
    /// Explicit unordered ingestion into an exclusively owned temporary spool.
    /// The spool is not itself a valid published chunk. Use finish_sorted_into
    /// with a distinct destination. The global coefficient table remains in RAM;
    /// only keys/ranges, not per-record algebra, are indexed alongside it.
    pub fn new_unordered(
        spool: W,
        binding: &ApplicationCacheBinding,
        context: &'a CoefficientContext,
        limits: ApplicationCacheIoLimits,
    ) -> Result<Self, BinaryIoError> {
        let mut writer = Self::new(spool, binding, context, limits)?;
        writer.unordered = Some(UnorderedRows {
            rows: BTreeMap::new(),
            index_bytes: 0,
            identical_duplicates: 0,
        });
        Ok(writer)
    }

    /// Append a row in any key order. Duplicate keys are accepted only when
    /// their complete encoded rows agree, including guard order/multiplicity.
    /// All comparisons use this same native coefficient interner; conflicting
    /// coefficients or guards poison finalization. No per-row state is exported.
    pub fn push_unordered(&mut self, record: &ApplicationCacheRecord) -> Result<(), BinaryIoError> {
        if self.poisoned {
            return Err(BinaryIoError::Invalid("cache chunk writer is poisoned"));
        }
        self.poisoned = true;
        self.push_unordered_inner(record)?;
        self.poisoned = false;
        Ok(())
    }

    fn push_unordered_inner(
        &mut self,
        record: &ApplicationCacheRecord,
    ) -> Result<(), BinaryIoError> {
        resource_checkpoint()?;
        let rows = self
            .unordered
            .as_ref()
            .ok_or(BinaryIoError::Invalid("ordered cache writer requires push"))?;
        let existing = rows.rows.get(&record.key).cloned();
        let count = self
            .records
            .checked_add(usize::from(existing.is_none()))
            .ok_or(BinaryIoError::Invalid("cache record count overflow"))?;
        self.validate_record(record, count)?;
        if let Some(range) = existing {
            let end = self.output.position()?;
            self.output
                .inner
                .seek(SeekFrom::Start(range.start))
                .map_err(native_io_error)?;
            {
                let length = range.end - range.start;
                let reference = (&mut self.output.inner).take(length);
                let mut comparison = RowComparison::new(reference, length);
                encode_row(&mut comparison, &mut self.table, record)?;
                comparison.finish()?;
            }
            self.output
                .inner
                .seek(SeekFrom::Start(end))
                .map_err(native_io_error)?;
            let rows = self.unordered.as_mut().expect("unordered mode checked");
            rows.identical_duplicates = rows
                .identical_duplicates
                .checked_add(1)
                .ok_or(BinaryIoError::Invalid("cache duplicate count overflow"))?;
            return Ok(());
        }
        // Match the existing exact-key scan's conservative per-tree-entry
        // allowance. Native/allocator scratch remains governed by ambient RSS.
        let charge = std::mem::size_of::<(ApplicationCacheKey, Range<u64>)>()
            .checked_add(record.key.owned_heap_bytes())
            .and_then(|n| n.checked_add(128))
            .ok_or(BinaryIoError::Invalid("unordered cache row index overflow"))?;
        let index_bytes = rows
            .index_bytes
            .checked_add(charge)
            .ok_or(BinaryIoError::Invalid("unordered cache row index overflow"))?;
        check_limit(
            "unordered cache row index bytes",
            index_bytes,
            self.limits.max_index_owned_bytes,
        )?;
        resource_allocation(charge)?;
        let indexed_key = record.key.clone();
        let start = self.output.position()?;
        encode_row(&mut self.output, &mut self.table, record)?;
        let end = self.output.position()?;
        let rows = self.unordered.as_mut().expect("unordered mode checked");
        rows.rows.insert(indexed_key, start..end);
        rows.index_bytes = index_bytes;
        self.records = count;
        Ok(())
    }

    /// Emit sorted unique rows into a DISTINCT, exclusively owned destination,
    /// then append the original single global table. Input and output must not
    /// alias storage. The caller owns both temporary files and atomic publication.
    /// Coefficient IDs follow input encounter order; output row order is canonical.
    /// A failed copy or table write leaves partial output and returns no success.
    pub fn finish_sorted_into<W2: Write + Seek>(
        mut self,
        destination: W2,
    ) -> Result<(W2, ApplicationCacheChunkWriteStatistics), BinaryIoError> {
        resource_checkpoint()?;
        if self.poisoned {
            return Err(BinaryIoError::Invalid("cache chunk writer is poisoned"));
        }
        let rows = self.unordered.take().ok_or(BinaryIoError::Invalid(
            "ordered cache writer requires finish",
        ))?;
        let input_records = self
            .records
            .checked_add(rows.identical_duplicates)
            .ok_or(BinaryIoError::Invalid("cache input count overflow"))?;
        let mut sorted = ApplicationCacheChunkWriter::new(
            destination,
            &self.binding,
            self.context,
            self.limits,
        )?;
        let mut buffer = [0u8; 16 * 1024];
        for (_, range) in rows.rows {
            resource_checkpoint()?;
            self.output
                .inner
                .seek(SeekFrom::Start(range.start))
                .map_err(native_io_error)?;
            let mut remaining = range.end - range.start;
            while remaining != 0 {
                resource_checkpoint()?;
                let requested = usize::try_from(remaining.min(buffer.len() as u64))
                    .map_err(|_| BinaryIoError::Invalid("cache row copy size overflow"))?;
                self.output
                    .inner
                    .read_exact(&mut buffer[..requested])
                    .map_err(native_io_error)?;
                put(&mut sorted.output, &buffer[..requested])?;
                remaining -= requested as u64;
            }
        }
        sorted.table = self.table;
        sorted.records = self.records;
        let (output, mut statistics) = sorted.finish()?;
        statistics.input_records = input_records;
        statistics.identical_duplicates = rows.identical_duplicates;
        statistics.unordered_spool_bytes = u64::try_from(self.output.bytes)
            .map_err(|_| BinaryIoError::Invalid("cache spool length exceeds u64"))?;
        statistics.unordered_index_owned_bytes = rows.index_bytes;
        Ok((output, statistics))
    }
}

fn encode_row(
    output: &mut impl Write,
    table: &mut CoefficientTableBuilder,
    record: &ApplicationCacheRecord,
) -> Result<(), BinaryIoError> {
    key(output, &record.key)?;
    write_length(output, record.terms.len())?;
    for (target, coefficient) in &record.terms {
        resource_checkpoint()?;
        powers(output, target.powers())?;
        put(
            output,
            &(table.intern(coefficient)?.index() as u32).to_le_bytes(),
        )?;
    }
    write_length(output, record.nonzero_conditions.len())?;
    for condition in &record.nonzero_conditions {
        resource_checkpoint()?;
        put(
            output,
            &(table.intern(condition)?.index() as u32).to_le_bytes(),
        )?;
    }
    Ok(())
}

/// Compare a candidate row against bounded existing bytes. The fixed buffer
/// prevents one read syscall per tiny integer while retaining no row payload.
struct RowComparison<R> {
    reference: R,
    buffer: [u8; 16 * 1024],
    offset: usize,
    filled: usize,
    written: u64,
    expected: u64,
}
impl<R: Read> RowComparison<R> {
    fn new(reference: R, expected: u64) -> Self {
        Self {
            reference,
            buffer: [0; 16 * 1024],
            offset: 0,
            filled: 0,
            written: 0,
            expected,
        }
    }
    fn finish(self) -> Result<(), BinaryIoError> {
        if self.written != self.expected {
            return Err(BinaryIoError::Invalid(
                "conflicting cache records for one exact key",
            ));
        }
        Ok(())
    }
}
impl<R: Read> Write for RowComparison<R> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        resource_checkpoint().map_err(io::Error::other)?;
        let written = self
            .written
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("cache row comparison length overflow"))?;
        if written > self.expected {
            return Err(io::Error::other(BinaryIoError::Invalid(
                "conflicting cache records for one exact key",
            )));
        }
        let mut remaining = bytes;
        while !remaining.is_empty() {
            if self.offset == self.filled {
                resource_checkpoint().map_err(io::Error::other)?;
                self.filled = self.reference.read(&mut self.buffer)?;
                self.offset = 0;
                if self.filled == 0 {
                    return Err(io::Error::other(BinaryIoError::Invalid(
                        "truncated unordered cache row spool",
                    )));
                }
            }
            let count = remaining.len().min(self.filled - self.offset);
            if remaining[..count] != self.buffer[self.offset..self.offset + count] {
                return Err(io::Error::other(BinaryIoError::Invalid(
                    "conflicting cache records for one exact key",
                )));
            }
            self.offset += count;
            remaining = &remaining[count..];
        }
        self.written = written;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Records must be sorted and unique. This snapshots completed mathematical
/// payload only; it does not inspect or serialize an in-flight application.
pub fn encode_application_cache_chunk(
    binding: &ApplicationCacheBinding,
    records: &[ApplicationCacheRecord],
    context: &CoefficientContext,
    limits: ApplicationCacheIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    let mut writer =
        ApplicationCacheChunkWriter::new(MemoryOutput::default(), binding, context, limits)?;
    for record in records {
        writer.push(record)?;
    }
    let (output, _) = writer.finish()?;
    Ok(output.bytes)
}

fn put(writer: &mut impl Write, bytes: &[u8]) -> Result<(), BinaryIoError> {
    resource_checkpoint()?;
    writer.write_all(bytes).map_err(native_io_error)
}
fn frame(writer: &mut impl Write, bytes: &[u8]) -> Result<(), BinaryIoError> {
    write_length(writer, bytes.len())?;
    put(writer, bytes)
}
fn key(writer: &mut impl Write, value: &ApplicationCacheKey) -> Result<(), BinaryIoError> {
    put(
        writer,
        &[match value.stage {
            ApplicationCacheStage::GuardedRoot => 0,
            ApplicationCacheStage::Route => 1,
            ApplicationCacheStage::Apply => 2,
        }],
    )?;
    if value.stage == ApplicationCacheStage::Apply {
        for &active in &value.owner {
            resource_checkpoint()?;
            put(writer, &[u8::from(active)])?;
        }
    }
    powers(writer, &value.powers)
}
fn powers(writer: &mut impl Write, values: &[i64]) -> Result<(), BinaryIoError> {
    for value in values {
        resource_checkpoint()?;
        put(writer, &value.to_le_bytes())?;
    }
    Ok(())
}

struct LimitedOutput<W> {
    inner: W,
    start: u64,
    bytes: usize,
    limit: usize,
}
impl<W: Write + Seek> LimitedOutput<W> {
    fn new(mut inner: W, limit: usize) -> Result<Self, BinaryIoError> {
        let start = inner.stream_position().map_err(native_io_error)?;
        Ok(Self {
            inner,
            start,
            bytes: 0,
            limit,
        })
    }
    fn position(&self) -> Result<u64, BinaryIoError> {
        self.start
            .checked_add(
                u64::try_from(self.bytes)
                    .map_err(|_| BinaryIoError::Invalid("cache chunk length exceeds u64"))?,
            )
            .ok_or(BinaryIoError::Invalid("cache chunk output offset overflow"))
    }
}
impl<W: Write> Write for LimitedOutput<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        resource_checkpoint().map_err(io::Error::other)?;
        let requested = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("cache chunk byte count overflow"))?;
        check_limit("cache chunk bytes", requested, self.limit).map_err(io::Error::other)?;
        let written = self.inner.write(bytes)?;
        self.bytes = self
            .bytes
            .checked_add(written)
            .ok_or_else(|| io::Error::other("cache chunk byte count overflow"))?;
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Fallible, resource-accounted buffer for the backwards-compatible Vec API.
#[derive(Default)]
struct MemoryOutput {
    bytes: Vec<u8>,
    position: usize,
}
impl Write for MemoryOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        resource_checkpoint().map_err(io::Error::other)?;
        let end = self
            .position
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("cache memory output offset overflow"))?;
        if end > self.bytes.len() {
            let growth = end - self.bytes.len();
            resource_allocation(growth).map_err(io::Error::other)?;
            self.bytes
                .try_reserve(growth)
                .map_err(|_| io::Error::other("cache memory output allocation failed"))?;
            self.bytes.resize(end, 0);
        }
        self.bytes[self.position..end].copy_from_slice(bytes);
        self.position = end;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for MemoryOutput {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        let position = match from {
            SeekFrom::Start(n) => i128::from(n),
            SeekFrom::Current(n) => self.position as i128 + i128::from(n),
            SeekFrom::End(n) => self.bytes.len() as i128 + i128::from(n),
        };
        if position < 0 || position > self.bytes.len() as i128 {
            return Err(io::Error::other("cache memory seek outside written bytes"));
        }
        self.position = usize::try_from(position).map_err(io::Error::other)?;
        u64::try_from(self.position).map_err(io::Error::other)
    }
}

#[cfg(test)]
mod tests;
