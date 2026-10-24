//! Explicit opt-in two-pass encoding for immutable integer metadata. Bincode
//! owns sizing and encoding; this adapter only enforces the existing byte cap.
use super::*;
use bincode::enc::{
    EncoderImpl,
    write::{SizeWriter, Writer},
};

/// Encode repeatable immutable metadata into the unchanged framed format.
///
/// The value's Encode implementation is called twice and MUST produce the
/// same bytes both times. Use encode_metadata_frame for arbitrary/stateful
/// Encode implementations (including serializers with mutable symbol state).
/// A changed encoded length is rejected, but equal-length mutation is not
/// detected. No semantic or closure authority is granted by this opt-in API.
///
/// The counting pass checks every write before updating Bincode's SizeWriter;
/// the complete frame is then reserved fallibly and the payload encoded into
/// a bounded slice. No unbounded allocation precedes the caller's byte grant.
pub fn encode_metadata_frame_repeatable(
    magic: &[u8],
    value: &impl bincode::Encode,
    limit: usize,
) -> GeneratedWorkResult<Vec<u8>> {
    checkpoint()?;
    let framing = magic
        .len()
        .checked_add(32)
        .ok_or_else(|| resource("finite frame overhead overflow"))?;
    let payload_limit = limit
        .checked_sub(framing)
        .ok_or_else(|| resource("finite frame allowance"))?;
    let mut counter = EncoderImpl::new(
        BoundedSizeWriter {
            inner: SizeWriter::default(),
            limit: payload_limit,
        },
        bincode::config::standard(),
    );
    value.encode(&mut counter).map_err(encode_resource)?;
    let payload_bytes = counter.into_writer().inner.bytes_written;
    let frame_bytes = framing
        .checked_add(payload_bytes)
        .filter(|&bytes| bytes <= limit)
        .ok_or_else(|| resource("finite frame size exceeds allowance"))?;
    let payload_end = frame_bytes - 32;
    let mut bytes = Vec::new();
    allocation(frame_bytes)?;
    bytes.try_reserve_exact(frame_bytes).map_err(resource)?;
    bytes.resize(payload_end, 0);
    bytes[..magic.len()].copy_from_slice(magic);
    let mut encoder = EncoderImpl::new(
        ResourceSliceWriter {
            bytes: &mut bytes[magic.len()..],
            written: 0,
        },
        bincode::config::standard(),
    );
    value.encode(&mut encoder).map_err(encode_resource)?;
    let written = encoder.into_writer().written;
    if written != payload_bytes {
        return Err(resource(
            "repeatable metadata changed encoded length between passes",
        ));
    }
    let digest = blake3::hash(&bytes);
    // Full frame, including this digest, was reserved before encoding.
    bytes.extend_from_slice(digest.as_bytes());
    Ok(bytes)
}

struct BoundedSizeWriter {
    inner: SizeWriter,
    limit: usize,
}
impl Writer for BoundedSizeWriter {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        checkpoint().map_err(budget_error)?;
        let length = self
            .inner
            .bytes_written
            .checked_add(bytes.len())
            .ok_or(bincode::error::EncodeError::Other("finite frame overflow"))?;
        if length > self.limit {
            return Err(bincode::error::EncodeError::Other(
                "finite frame allowance exceeded",
            ));
        }
        // SizeWriter's public implementation uses unchecked addition. The
        // preceding check makes its update safe even in optimized builds.
        self.inner.write(bytes)
    }
}

fn budget_error(error: GeneratedWorkError) -> bincode::error::EncodeError {
    bincode::error::EncodeError::Io {
        inner: std::io::Error::other(error),
        index: 0,
    }
}

struct ResourceSliceWriter<'a> {
    bytes: &'a mut [u8],
    written: usize,
}
impl Writer for ResourceSliceWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> Result<(), bincode::error::EncodeError> {
        checkpoint().map_err(budget_error)?;
        let end = self
            .written
            .checked_add(bytes.len())
            .ok_or(bincode::error::EncodeError::UnexpectedEnd)?;
        let target = self
            .bytes
            .get_mut(self.written..end)
            .ok_or(bincode::error::EncodeError::UnexpectedEnd)?;
        target.copy_from_slice(bytes);
        self.written = end;
        Ok(())
    }
}
