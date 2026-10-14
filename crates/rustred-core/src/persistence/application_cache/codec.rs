//! Only integer keys/counts/IDs are RustRed framing. All algebra bytes and
//! symbol remapping use the existing generated native coefficient-table codec.
use std::{io::Write, ops::Range, sync::Arc};

use super::*;
use crate::persistence::error::check_limit;
use crate::persistence::limits::CappedWriter;
use crate::persistence::native::{read_length, write_length};
use crate::persistence::{CoefficientId, CoefficientTableBuilder, LazyDecodedCoefficientTable};

const MAGIC: &[u8; 16] = b"RR-CACHE-CHNK-01";

fn native(error: impl std::fmt::Display) -> BinaryIoError {
    BinaryIoError::Native(error.to_string())
}
fn put(writer: &mut CappedWriter, bytes: &[u8]) -> Result<(), BinaryIoError> {
    writer.write_all(bytes).map_err(native)
}
fn frame(writer: &mut CappedWriter, bytes: &[u8]) -> Result<(), BinaryIoError> {
    write_length(writer, bytes.len())?;
    put(writer, bytes)
}
fn key(writer: &mut CappedWriter, value: &ApplicationCacheKey) -> Result<(), BinaryIoError> {
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
            put(writer, &[u8::from(active)])?;
        }
    }
    powers(writer, &value.powers)
}
fn powers(writer: &mut CappedWriter, values: &[i64]) -> Result<(), BinaryIoError> {
    for value in values {
        put(writer, &value.to_le_bytes())?;
    }
    Ok(())
}

/// Records must be sorted and unique. This snapshots completed mathematical
/// payload only; it does not inspect or serialize an in-flight application.
pub fn encode_application_cache_chunk(
    binding: &ApplicationCacheBinding,
    records: &[ApplicationCacheRecord],
    context: &CoefficientContext,
    limits: ApplicationCacheIoLimits,
) -> Result<Vec<u8>, BinaryIoError> {
    binding.validate(limits)?;
    check_limit("cache records", records.len(), limits.max_records)?;
    let mut out = CappedWriter::new("cache chunk bytes", limits.binary.max_program_bytes);
    put(&mut out, MAGIC)?;
    put(&mut out, &binding.artifact_digest)?;
    frame(&mut out, binding.family_fingerprint.as_bytes())?;
    frame(&mut out, binding.layer.as_bytes())?;
    write_length(&mut out, binding.physical_arity)?;
    write_length(&mut out, records.len())?;
    let mut table = CoefficientTableBuilder::new(limits.binary);
    let mut previous = None;
    for record in records {
        record.key.validate(binding.physical_arity)?;
        if previous.is_some_and(|key| key >= &record.key) {
            return Err(BinaryIoError::Invalid(
                "cache keys are not strictly ordered",
            ));
        }
        previous = Some(&record.key);
        check_limit(
            "cache record terms",
            record.terms.len(),
            limits.max_terms_per_record,
        )?;
        check_limit(
            "cache record guards",
            record.nonzero_conditions.len(),
            limits.max_guards_per_record,
        )?;
        let (terms, bytes) = record.coefficient_census()?;
        check_limit(
            "decoded cache coefficient terms",
            terms,
            limits.max_decoded_coefficient_terms,
        )?;
        check_limit(
            "decoded cache coefficient bytes",
            bytes,
            limits.max_decoded_coefficient_bytes,
        )?;
        key(&mut out, &record.key)?;
        write_length(&mut out, record.terms.len())?;
        for (target, coefficient) in &record.terms {
            if target.powers().len() != binding.physical_arity || coefficient.is_zero() {
                return Err(BinaryIoError::Invalid(
                    "invalid or zero cache terminal term",
                ));
            }
            validate_coefficient(coefficient, context, limits)?;
            powers(&mut out, target.powers())?;
            put(
                &mut out,
                &(table.intern(coefficient)?.index() as u32).to_le_bytes(),
            )?;
        }
        write_length(&mut out, record.nonzero_conditions.len())?;
        for condition in &record.nonzero_conditions {
            validate_coefficient(condition, context, limits)?;
            if condition.is_zero() {
                return Err(BinaryIoError::Invalid("vanished cache guard"));
            }
            put(
                &mut out,
                &(table.intern(condition)?.index() as u32).to_le_bytes(),
            )?;
        }
    }
    let encoded = table.finish()?;
    frame(&mut out, &encoded.state)?;
    frame(&mut out, &encoded.atoms)?;
    Ok(out.finish())
}

struct Row {
    key: ApplicationCacheKey,
    terms: Range<usize>,
    term_count: usize,
    guards: Range<usize>,
    guard_count: usize,
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
        expected.validate(limits)?;
        check_limit(
            "cache chunk bytes",
            bytes.len(),
            limits.binary.max_program_bytes,
        )?;
        let mut read = Cursor(bytes);
        if read.take(MAGIC.len())? != MAGIC || read.take(32)? != expected.artifact_digest {
            return Err(BinaryIoError::Invalid(
                "cache magic or artifact identity mismatch",
            ));
        }
        if read.frame(limits.max_identity_bytes)? != expected.family_fingerprint.as_bytes()
            || read.frame(limits.max_identity_bytes)? != expected.layer.as_bytes()
            || read.count(limits.max_arity, "cache arity")? != expected.physical_arity
        {
            return Err(BinaryIoError::Invalid(
                "cache family, layer or arity mismatch",
            ));
        }
        let count = read.count(limits.max_records, "cache records")?;
        let mut rows = Vec::<Row>::new();
        rows.try_reserve_exact(count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "cache row index",
                requested: count,
            })?;
        for _ in 0..count {
            let key = read.key(expected.physical_arity)?;
            if rows.last().is_some_and(|previous| previous.key >= key) {
                return Err(BinaryIoError::Invalid(
                    "cache keys are not strictly ordered",
                ));
            }
            let term_count = read.count(limits.max_terms_per_record, "cache record terms")?;
            let term_bytes = expected
                .physical_arity
                .checked_mul(8)
                .and_then(|n| n.checked_add(4))
                .and_then(|n| n.checked_mul(term_count))
                .ok_or(BinaryIoError::Invalid("cache term framing overflow"))?;
            let start = bytes.len() - read.0.len();
            read.take(term_bytes)?;
            let terms = start..start + term_bytes;
            let guard_count = read.count(limits.max_guards_per_record, "cache record guards")?;
            let guard_bytes = guard_count
                .checked_mul(4)
                .ok_or(BinaryIoError::Invalid("cache guard framing overflow"))?;
            let start = bytes.len() - read.0.len();
            read.take(guard_bytes)?;
            rows.push(Row {
                key,
                terms,
                term_count,
                guards: start..start + guard_bytes,
                guard_count,
            });
        }
        let state = read.frame(limits.binary.max_state_bytes)?;
        let atoms = read.frame(limits.binary.max_total_atom_bytes)?;
        if !read.0.is_empty() {
            return Err(BinaryIoError::Invalid("trailing cache chunk bytes"));
        }
        Ok(Self {
            bytes,
            rows,
            state,
            atoms,
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
        let table = LazyDecodedCoefficientTable::new(
            Arc::from(self.state),
            Arc::from(self.atoms),
            self.limits.binary,
        )?;
        let mut census = crate::reduction::CacheWeight::default();
        let mut coefficient = |id: u32| -> Result<Coefficient, BinaryIoError> {
            let value = table.coefficient(CoefficientId::try_from_index(id as usize)?)?;
            validate_coefficient(&value, context, self.limits)?;
            census = census
                .checked_add(coefficient_cache_weight([value.as_ref()]).map_err(native)?)
                .map_err(native)?;
            check_limit(
                "decoded cache coefficient terms",
                census.coefficient_terms,
                self.limits.max_decoded_coefficient_terms,
            )?;
            check_limit(
                "decoded cache coefficient bytes",
                census.coefficient_bytes,
                self.limits.max_decoded_coefficient_bytes,
            )?;
            Ok((*value).clone())
        };
        let mut terms = BTreeMap::new();
        let mut read = Cursor(&self.bytes[row.terms.clone()]);
        for _ in 0..row.term_count {
            let target =
                IntegralKey::try_from_preallocated(read.powers(self.arity)?).map_err(native)?;
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
            terms.insert(target, value);
        }
        let mut nonzero_conditions = Vec::new();
        nonzero_conditions
            .try_reserve_exact(row.guard_count)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "cache guards",
                requested: row.guard_count,
            })?;
        let mut read = Cursor(&self.bytes[row.guards.clone()]);
        for _ in 0..row.guard_count {
            let value = coefficient(read.id()?)?;
            if value.is_zero() {
                return Err(BinaryIoError::Invalid("vanished cache guard"));
            }
            nonzero_conditions.push(value);
        }
        Ok(Some(ApplicationCacheRecord {
            key: row.key.clone(),
            terms,
            nonzero_conditions,
        }))
    }
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], BinaryIoError> {
        let result = self
            .0
            .get(..count)
            .ok_or(BinaryIoError::Invalid("truncated cache chunk"))?;
        self.0 = &self.0[count..];
        Ok(result)
    }
    fn count(&mut self, limit: usize, resource: &'static str) -> Result<usize, BinaryIoError> {
        let count = read_length(&mut self.0)?;
        check_limit(resource, count, limit)?;
        Ok(count)
    }
    fn frame(&mut self, limit: usize) -> Result<&'a [u8], BinaryIoError> {
        let count = self.count(limit, "cache frame bytes")?;
        self.take(count)
    }
    fn powers(&mut self, arity: usize) -> Result<Vec<i64>, BinaryIoError> {
        let mut powers = Vec::new();
        powers
            .try_reserve_exact(arity)
            .map_err(|_| BinaryIoError::Allocation {
                resource: "cache powers",
                requested: arity,
            })?;
        for _ in 0..arity {
            powers.push(i64::from_le_bytes(
                self.take(8)?.try_into().expect("checked slice"),
            ));
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
            self.take(arity)?
                .iter()
                .map(|&value| match value {
                    0 => Ok(false),
                    1 => Ok(true),
                    _ => Err(BinaryIoError::Invalid("nonboolean cache owner")),
                })
                .collect::<Result<_, _>>()?
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
            self.take(4)?.try_into().expect("checked slice"),
        ))
    }
}
