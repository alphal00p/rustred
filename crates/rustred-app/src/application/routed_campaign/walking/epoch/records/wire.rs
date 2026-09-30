//! Framed Epoch record schema1. The small authority and optional diagnostic
//! payload use the existing bincode codec. Framing is fixed little-endian:
//! `ERB1`, authority-byte-count:u32, diagnostic-byte-count:u32, then both bodies.
//! Every consumer hashes the same opened stream it decodes. Restore may skip
//! diagnostics through that reader; it never reopens an unauthenticated body.
use super::typed::{Authority, Diagnostics, Record};
use std::io::{self, Read, Write};

pub(in crate::application::routed_campaign::walking) const RECORD_SCHEMA: u32 = 1;
pub(in crate::application::routed_campaign::walking) const HEADER_BYTES: usize = 12;
const MAGIC: [u8; 4] = *b"ERB1";
// Covers the existing bounded G2 residual/anchor representation at every
// supported arity. This is a wire safety limit, not a geometry truncation.
pub(in crate::application::routed_campaign::walking) const MAX_AUTHORITY_BYTES: usize = 128 << 20;
pub(in crate::application::routed_campaign::walking) const MAX_DIAGNOSTIC_BYTES: usize = 1 << 30;

fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

struct BoundedTail<'a> {
    output: &'a mut Vec<u8>,
    start: usize,
    limit: usize,
}
impl Write for BoundedTail<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .output
            .len()
            .saturating_sub(self.start)
            .checked_add(bytes.len())
            .is_none_or(|n| n > self.limit)
        {
            return Err(invalid("epoch record encoded size limit"));
        }
        self.output
            .try_reserve(bytes.len())
            .map_err(|_| io::Error::other("epoch record buffer allocation"))?;
        self.output.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Append one whole frame directly to the writer's reusable byte batch. A
/// failed encoding leaves the batch unchanged, never a publishable prefix.
pub(in crate::application::routed_campaign::walking) fn append(
    record: &Record,
    output: &mut Vec<u8>,
) -> io::Result<()> {
    record.validate().map_err(invalid)?;
    let first = output.len();
    output
        .try_reserve(HEADER_BYTES)
        .map_err(|_| io::Error::other("epoch record frame allocation"))?;
    output.extend_from_slice(&[0; HEADER_BYTES]);
    let result = (|| {
        let begin = output.len();
        bincode::encode_into_std_write(
            &record.authority,
            &mut BoundedTail {
                output,
                start: begin,
                limit: MAX_AUTHORITY_BYTES,
            },
            bincode::config::standard(),
        )
        .map_err(|e| invalid(format!("epoch record authority encoding: {e}")))?;
        let authority_bytes = output.len() - begin;
        let begin = output.len();
        bincode::encode_into_std_write(
            &record.diagnostics,
            &mut BoundedTail {
                output,
                start: begin,
                limit: MAX_DIAGNOSTIC_BYTES,
            },
            bincode::config::standard(),
        )
        .map_err(|e| invalid(format!("epoch record diagnostic encoding: {e}")))?;
        let diagnostic_bytes = output.len() - begin;
        output[first..first + 4].copy_from_slice(&MAGIC);
        output[first + 4..first + 8].copy_from_slice(&(authority_bytes as u32).to_le_bytes());
        output[first + 8..first + 12].copy_from_slice(&(diagnostic_bytes as u32).to_le_bytes());
        Ok(())
    })();
    if result.is_err() {
        output.truncate(first);
    }
    result
}

fn body(input: &mut impl Read, count: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(count)
        .map_err(|_| io::Error::other("epoch record decode allocation"))?;
    bytes.resize(count, 0);
    input.read_exact(&mut bytes)?;
    Ok(bytes)
}

/// Decode one authority, leaving exactly the returned diagnostic byte count
/// unread. The caller must consume/skip it before reading another frame.
pub(in crate::application::routed_campaign::walking) fn authority(
    input: &mut impl Read,
) -> io::Result<(Authority, usize)> {
    let mut header = [0; HEADER_BYTES];
    input.read_exact(&mut header)?;
    if header[..4] != MAGIC {
        return Err(invalid("epoch record frame magic/schema"));
    }
    let auth = u32::from_le_bytes(header[4..8].try_into().expect("four bytes")) as usize;
    let diag = u32::from_le_bytes(header[8..12].try_into().expect("four bytes")) as usize;
    if auth == 0 || auth > MAX_AUTHORITY_BYTES || diag == 0 || diag > MAX_DIAGNOSTIC_BYTES {
        return Err(invalid("epoch record frame byte bounds"));
    }
    let bytes = body(input, auth)?;
    let (value, used): (Authority, usize) = bincode::decode_from_slice(
        &bytes,
        bincode::config::standard().with_limit::<MAX_AUTHORITY_BYTES>(),
    )
    .map_err(|e| invalid(format!("epoch record authority decoding: {e}")))?;
    if used != bytes.len() {
        return Err(invalid("epoch record authority trailing bytes"));
    }
    value.validate().map_err(invalid)?;
    Ok((value, diag))
}

/// Fixed scratch, consuming the authenticated stream, not seeking past bytes.
pub(in crate::application::routed_campaign::walking) fn skip_diagnostics(
    input: &mut impl Read,
    mut count: usize,
) -> io::Result<()> {
    let mut scratch = [0; 32 * 1024];
    while count > 0 {
        let n = count.min(scratch.len());
        input.read_exact(&mut scratch[..n])?;
        count -= n;
    }
    Ok(())
}

pub(in crate::application::routed_campaign::walking) fn read(
    input: &mut impl Read,
) -> io::Result<Record> {
    let (authority, count) = authority(input)?;
    let bytes = body(input, count)?;
    let (diagnostics, used): (Diagnostics, usize) = bincode::decode_from_slice(
        &bytes,
        bincode::config::standard().with_limit::<MAX_DIAGNOSTIC_BYTES>(),
    )
    .map_err(|e| invalid(format!("epoch record diagnostic decoding: {e}")))?;
    if used != bytes.len() {
        return Err(invalid("epoch record diagnostic trailing bytes"));
    }
    let record = Record {
        authority,
        diagnostics,
    };
    record.validate().map_err(invalid)?;
    Ok(record)
}

#[cfg(test)]
mod tests;
