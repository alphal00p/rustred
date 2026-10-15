use super::*;
use std::io::Write;

pub const FRAME_LIMIT: usize = 64 * 1024 * 1024;
pub const PAGE_MAGIC: &[u8] = b"RUSTRED-FINITE-WORK-1\0";

#[derive(bincode::Encode, bincode::Decode)]
pub struct GeneratedWorkPageRecord {
    pub execution: GeneratedExecutionBinding,
    pub records: Vec<GeneratedWorkKey>,
}

/// Private native-decoded payload. A provider cannot manufacture a successful
/// query answer, alter its keys, or substitute another descriptor after decode.
pub struct ValidatedWorkPage {
    execution: GeneratedExecutionBinding,
    digest: GeneratedWorkDigest,
    encoded_bytes: u64,
    records: Vec<GeneratedWorkKey>,
    payload_bytes: usize,
}
impl ValidatedWorkPage {
    pub fn decode_generated(
        bytes: &[u8],
        expected: &GeneratedWorkPageRef,
        execution: &GeneratedExecutionBinding,
        max_encoded_bytes: usize,
        max_records: usize,
    ) -> GeneratedWorkResult<Self> {
        if bytes.len() as u64 != expected.object.bytes
            || blake3::hash(bytes).as_bytes() != &expected.object.digest
        {
            return Err(corrupt("finite page object digest/length mismatch"));
        }
        let page: GeneratedWorkPageRecord =
            decode_metadata_frame(PAGE_MAGIC, bytes, max_encoded_bytes)?;
        let payload_bytes = validate_page_record(&page, expected, execution, max_records)?;
        Ok(Self {
            execution: page.execution,
            digest: expected.object.digest,
            encoded_bytes: bytes.len() as u64,
            records: page.records,
            payload_bytes,
        })
    }
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }
    pub fn matches(
        &self,
        expected: &GeneratedWorkPageRef,
        execution: &GeneratedExecutionBinding,
    ) -> bool {
        self.execution == *execution
            && self.digest == expected.object.digest
            && self.encoded_bytes == expected.object.bytes
            && self.records.len() as u64 == expected.records
            && self.records.first() == Some(&expected.first)
            && self.records.last() == Some(&expected.last)
    }
    pub(super) fn contains(&self, key: &GeneratedWorkKey) -> bool {
        self.records.binary_search(key).is_ok()
    }
}

pub fn work_payload_bytes(
    records: &[GeneratedWorkKey],
    capacity: usize,
) -> GeneratedWorkResult<usize> {
    let mut bytes = capacity
        .checked_mul(std::mem::size_of::<GeneratedWorkKey>())
        .ok_or_else(|| resource("page capacity overflow"))?;
    for record in records {
        bytes = bytes
            .checked_add(record.heap_bytes()?)
            .ok_or_else(|| resource("page payload overflow"))?;
    }
    Ok(bytes)
}
pub fn validate_page_record(
    page: &GeneratedWorkPageRecord,
    reference: &GeneratedWorkPageRef,
    execution: &GeneratedExecutionBinding,
    max_records: usize,
) -> GeneratedWorkResult<usize> {
    if page.execution != *execution
        || page.records.len() as u64 != reference.records
        || page.records.first() != Some(&reference.first)
        || page.records.last() != Some(&reference.last)
        || page.records.is_empty()
        || page.records.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(corrupt(
            "finite page execution/range/sort/duplicate mismatch",
        ));
    }
    if page.records.len() > max_records {
        return Err(resource("finite page record allowance"));
    }
    for work in &page.records {
        work.validate(execution.physical_arity as usize)?;
    }
    work_payload_bytes(&page.records, page.records.capacity())
}

/// Shared bounded integer-metadata framing, not an algebra/closure certificate.
pub fn encode_metadata_frame(
    magic: &[u8],
    value: &impl bincode::Encode,
    limit: usize,
) -> GeneratedWorkResult<Vec<u8>> {
    if limit > FRAME_LIMIT || limit < magic.len() + 32 {
        return Err(resource("finite frame allowance"));
    }
    let mut writer = BoundedBytes {
        bytes: Vec::new(),
        limit: limit - 32,
    };
    writer.write_all(magic).map_err(resource)?;
    bincode::encode_into_std_write(value, &mut writer, bincode::config::standard())
        .map_err(resource)?;
    let digest = blake3::hash(&writer.bytes);
    writer.bytes.extend_from_slice(digest.as_bytes());
    Ok(writer.bytes)
}
pub fn decode_metadata_frame<T: bincode::Decode<()>>(
    magic: &[u8],
    bytes: &[u8],
    limit: usize,
) -> GeneratedWorkResult<T> {
    if limit > FRAME_LIMIT
        || bytes.len() > limit
        || bytes.len() < magic.len() + 32
        || !bytes.starts_with(magic)
    {
        return Err(corrupt("finite frame magic/length"));
    }
    let end = bytes.len() - 32;
    if blake3::hash(&bytes[..end]).as_bytes() != &bytes[end..] {
        return Err(corrupt("finite frame digest"));
    }
    let (value, consumed) = bincode::decode_from_slice(
        &bytes[magic.len()..end],
        bincode::config::standard().with_limit::<FRAME_LIMIT>(),
    )
    .map_err(|error| match error {
        bincode::error::DecodeError::LimitExceeded => resource(error),
        _ => corrupt(error),
    })?;
    if consumed != end - magic.len() {
        return Err(corrupt("trailing finite frame bytes"));
    }
    Ok(value)
}
struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("finite frame overflow"))?;
        if length > self.limit {
            return Err(std::io::Error::other("finite frame allowance exceeded"));
        }
        if length > self.bytes.capacity() {
            let capacity = length
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(std::io::Error::other)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
