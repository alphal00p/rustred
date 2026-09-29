//! Initial D-band anchor decoding reuses the existing codec one bounded
//! record at a time. Full anchor/ledger/edge/geometry validation is separate.
use super::super::super::anchors::{ANCHORS_VERSION, AnchorKind, AnchorMap, Lent};
use super::super::{Section, SectionReceipt, invalid};
use super::open_section;
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
    if receipt.section != Section::Anchors
        || watermark == u32::MAX
        || p0 > watermark
        || epoch >= super::super::super::ledger6::EPOCH_LIMIT
        || count > u64::from(watermark - p0)
    {
        return Err(invalid("epoch anchor inventory range"));
    }
    let mut reader = open_section::<N>(directory, receipt, count)?;
    // The admitted private format supports InitialDBand only: 10-byte
    // anchor header plus exactly 48 bytes per one-anchor/8-byte-cut record.
    if count.checked_mul(48).and_then(|body| body.checked_add(10)) != Some(reader.remaining()) {
        return Err(invalid("epoch initial anchor count/length differs"));
    }
    let mut version = [0; 2];
    reader.read_exact(&mut version)?;
    if u16::from_le_bytes(version) != ANCHORS_VERSION || reader.u64()? != count {
        return Err(invalid("epoch anchor codec header differs"));
    }
    let mut scratch = [0u8; 58];
    scratch[..2].copy_from_slice(&ANCHORS_VERSION.to_le_bytes());
    scratch[2..10].copy_from_slice(&1u64.to_le_bytes());
    let mut anchors = AnchorMap::default();
    let mut previous = None;
    for _ in 0..count {
        reader.read_exact(&mut scratch[10..])?;
        let row = &scratch[10..];
        if row[4] != AnchorKind::InitialDBand as u8
            || row[8..12] != 1u32.to_le_bytes()
            || row[12..16] != 8u32.to_le_bytes()
        {
            return Err(invalid(
                "unsupported epoch anchor kind or variable record shape",
            ));
        }
        // Counts/kind are bounded before entering the shared generic codec;
        // it checks canonical reserved bytes, stamps and exact consumption.
        let mut decoded = AnchorMap::decode(&scratch, N).map_err(io::Error::other)?;
        let record = decoded
            .pop()
            .ok_or_else(|| invalid("missing epoch anchor record"))?;
        if !decoded.is_empty()
            || record.node < p0
            || record.node >= watermark
            || previous.is_some_and(|id| id >= record.node)
            || record.dispatch_version > epoch
            || record.anchors[0].anchor >= p0
            || record.anchors[0].stamp.is_some()
            || record.anchors[0].lent != Lent::Full
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
