//! Transport only: the native Symbolica state-aware chunk is unchanged.
use std::io::{Read, Write};

use flate2::{Compression, bufread::GzDecoder, write::GzEncoder};

use super::{
    SavedApplicationCachePackLimits, fail, index, io_error, resource_allocation,
    resource_checkpoint,
};
use crate::AppError;

pub(super) fn encode(
    decoded: &[u8],
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<u8>, AppError> {
    resource_checkpoint()?;
    if decoded.len() > limits.chunk.binary.max_program_bytes {
        return Err(AppError::limit(
            "decoded cache chunk byte allowance exceeded",
        ));
    }
    let mut output = index::BoundedBytes::new(limits.max_encoded_chunk_bytes);
    let result = (|| {
        let mut encoder = GzEncoder::new(&mut output, Compression::fast());
        encoder.write_all(decoded)?;
        encoder.finish()?;
        Ok::<_, std::io::Error>(())
    })();
    if output.limit_exceeded() {
        return Err(AppError::limit(
            "compressed cache chunk byte allowance exceeded",
        ));
    }
    result.map_err(io_error)?;
    resource_checkpoint()?;
    Ok(output.finish())
}

pub(super) fn decode(
    encoded: &[u8],
    expected_bytes: u64,
    expected_digest: &[u8; 32],
    limits: SavedApplicationCachePackLimits,
) -> Result<Vec<u8>, AppError> {
    let length = usize::try_from(expected_bytes)
        .map_err(|_| AppError::limit("decoded cache chunk does not fit address space"))?;
    if encoded.len() > limits.max_encoded_chunk_bytes
        || length > limits.chunk.binary.max_program_bytes
    {
        return Err(AppError::limit("cache compression envelope exceeded"));
    }
    let mut output = Vec::new();
    resource_allocation(length)?;
    output.try_reserve_exact(length).map_err(fail)?;
    // bufread over &[u8] preserves the exact position after one gzip member.
    // The Read wrapper is intentionally not used: it may consume trailing data.
    let mut decoder = GzDecoder::new(encoded);
    let mut buffer = [0u8; 16 * 1024];
    while output.len() < length {
        resource_checkpoint()?;
        let requested = buffer.len().min(length - output.len());
        let read = decoder.read(&mut buffer[..requested]).map_err(fail)?;
        if read == 0 {
            return Err(fail("truncated decoded cache chunk"));
        }
        output.extend_from_slice(&buffer[..read]);
    }
    // Force CRC/trailer validation without allocating beyond the declared cap.
    if decoder.read(&mut buffer[..1]).map_err(fail)? != 0 {
        return Err(fail("decoded cache chunk exceeds its declared length"));
    }
    if !decoder.into_inner().is_empty() {
        return Err(fail("trailing gzip member or bytes in cache chunk"));
    }
    if blake3::hash(&output).as_bytes() != expected_digest {
        return Err(fail("decoded cache chunk digest mismatch"));
    }
    resource_checkpoint()?;
    Ok(output)
}
