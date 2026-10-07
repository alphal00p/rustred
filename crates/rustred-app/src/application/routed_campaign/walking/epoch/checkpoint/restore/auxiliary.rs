//! Initial D-band anchor decoding reuses the existing codec one bounded
//! record at a time. Full anchor/ledger/edge/geometry validation is separate.
use super::super::super::anchors::{
    ANCHORS_VERSION, AnchorKind, AnchorMap, Lent, MAX_ANCHORS, MAX_RESIDUAL_PIECES,
};
use super::super::{Section, SectionReceipt, invalid};
use super::open_section_with_arity;
use std::io::{self, Read};
use std::path::Path;

pub(super) fn anchors<const N: usize>(
    directory: &Path,
    receipt: &SectionReceipt,
    count: u64,
    watermark: u32,
    p0: u32,
    epoch: u64,
) -> io::Result<AnchorMap> {
    anchors_with_arity::<N>(directory, receipt, count, watermark, p0, epoch, N)
}

pub(super) fn anchors_with_arity<const N: usize>(
    directory: &Path,
    receipt: &SectionReceipt,
    count: u64,
    watermark: u32,
    p0: u32,
    epoch: u64,
    wire_arity: usize,
) -> io::Result<AnchorMap> {
    if receipt.section != Section::Anchors
        || watermark == u32::MAX
        || p0 > watermark
        || epoch >= super::super::super::ledger6::EPOCH_LIMIT
        || count > u64::from(watermark - p0)
    {
        return Err(invalid("epoch anchor inventory range"));
    }
    let (mut reader, _) = open_section_with_arity::<N>(directory, receipt, count, wire_arity)?;
    if count
        .checked_mul(44)
        .and_then(|body| body.checked_add(10))
        .is_none_or(|minimum| minimum > reader.remaining())
    {
        return Err(invalid("epoch anchor count exceeds section length"));
    }
    let mut version = [0; 2];
    reader.read_exact(&mut version)?;
    if u16::from_le_bytes(version) != ANCHORS_VERSION || reader.u64()? != count {
        return Err(invalid("epoch anchor codec header differs"));
    }
    let mut scratch = vec![0u8; 34];
    scratch[..2].copy_from_slice(&ANCHORS_VERSION.to_le_bytes());
    scratch[2..10].copy_from_slice(&1u64.to_le_bytes());
    let mut anchors = AnchorMap::default();
    let mut previous = None;
    for _ in 0..count {
        scratch.resize(34, 0);
        reader.read_exact(&mut scratch[10..34])?;
        let row = &scratch[10..34];
        let n = u32::from_le_bytes(row[8..12].try_into().expect("4")) as usize;
        let scope_len = u32::from_le_bytes(row[12..16].try_into().expect("4")) as usize;
        let tail = n
            .checked_mul(16)
            .and_then(|n| n.checked_add(scope_len))
            .ok_or_else(|| invalid("epoch anchor length overflow"))?;
        if n == 0
            || n > MAX_ANCHORS
            || n > watermark as usize
            || scope_len > 4 + MAX_RESIDUAL_PIECES * (18 + 4 * wire_arity)
            || tail as u64 > reader.remaining()
        {
            return Err(invalid("epoch anchor variable shape exceeds bounds"));
        }
        scratch
            .try_reserve(tail)
            .map_err(|_| invalid("epoch anchor allocation"))?;
        scratch.resize(34 + tail, 0);
        reader.read_exact(&mut scratch[34..])?;
        // Counts/kind are bounded before entering the shared generic codec;
        // it checks canonical reserved bytes, stamps and exact consumption.
        let mut decoded = AnchorMap::decode(&scratch, wire_arity).map_err(io::Error::other)?;
        let record = decoded
            .pop()
            .ok_or_else(|| invalid("missing epoch anchor record"))?;
        if !decoded.is_empty()
            || record.node < p0
            || record.node >= watermark
            || previous.is_some_and(|id| id >= record.node)
            || record.dispatch_version > epoch
            || record.anchors.iter().any(|a| a.anchor >= watermark)
            || record.kind == AnchorKind::InitialDBand
                && (record.anchors.len() != 1
                    || record.anchors[0].anchor >= p0
                    || record.anchors[0].stamp.is_some()
                    || record.anchors[0].lent != Lent::Full)
        {
            return Err(invalid(
                "epoch initial anchor order, range, version or provenance",
            ));
        }
        previous = Some(record.node);
        anchors.try_reserve(1).map_err(io::Error::other)?;
        anchors.push(record).map_err(io::Error::other)?;
    }
    reader.finish()?;
    Ok(anchors)
}

#[cfg(test)]
mod tests;
