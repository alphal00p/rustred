//! Stream the fixed-shape segment registry into its final Sidecar inventory.
//! The semantic visitor consumes the same reader which authenticates every
//! sealed body; there is no unchecked reopen or second full-body traversal.
use super::super::invalid;
use super::super::publication::FileRef;
use super::super::read::Budget;
use super::CheckedRead;
use crate::application::routed_campaign::walking::checkpoint::manifest::Segment;
use crate::application::routed_campaign::walking::epoch::{record_store, records::wire};
use serde::de::{self, DeserializeSeed, Deserializer, SeqAccess, Visitor};
use std::cell::Cell;
use std::fmt;
use std::io;
use std::path::Path;

// A descriptor has four u64 fields, one fixed records-<20 digits>.bin
// filename and a64-hex digest.512 exceeds its largest canonical encoding;
// it does NOT limit a record body or impose a topology/record-count cap.
const DESCRIPTOR_BYTES: u64 = 512;

struct Registry<'a> {
    budget: &'a Cell<u64>,
    generation: u64,
    segments: u64,
    records: u64,
}
impl<'de> DeserializeSeed<'de> for Registry<'_> {
    type Value = Vec<Segment>;
    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_seq(self)
    }
}
impl<'de> Visitor<'de> for Registry<'_> {
    type Value = Vec<Segment>;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("the epoch sealed-record segment array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
        let mut segments = Vec::new();
        let mut total = 0u64;
        let mut previous = 0;
        loop {
            // serde_json's reader decoder consumes at most one-byte lookahead;
            // each next_element gets a fresh fixed descriptor-sized budget.
            self.budget.set(DESCRIPTOR_BYTES);
            let Some(segment) = sequence.next_element::<Segment>()? else {
                break;
            };
            if segments.len() as u64 == self.segments
                || segment.generation <= previous
                || segment.generation > self.generation
                || segment.first != total
                || segment.count == 0
                || segment.file != record_store::file_name(segment.generation)
                || segment
                    .count
                    .checked_mul(wire::HEADER_BYTES as u64 + 2)
                    .is_none_or(|minimum| minimum > segment.bytes)
                || digest(&segment.blake3).is_err()
            {
                return Err(de::Error::custom(
                    "epoch record segment path, order, digest or inventory",
                ));
            }
            total = total
                .checked_add(segment.count)
                .filter(|&total| total <= self.records)
                .ok_or_else(|| de::Error::custom("epoch record segment total exceeds ledger"))?;
            previous = segment.generation;
            segments
                .try_reserve(1)
                .map_err(|_| de::Error::custom("epoch record segment allocation"))?;
            segments.push(segment);
        }
        if segments.len() as u64 != self.segments || total != self.records {
            return Err(de::Error::custom(
                "epoch record segments do not tile ledger inventory",
            ));
        }
        Ok(segments)
    }
}

fn digest(value: &str) -> io::Result<[u8; 32]> {
    if value.len() != 64 {
        return Err(invalid("epoch record segment digest length"));
    }
    let nibble = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(invalid("epoch record segment digest encoding")),
    };
    let mut decoded = [0; 32];
    for (out, pair) in decoded.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        *out = (nibble(pair[0])? << 4) | nibble(pair[1])?;
    }
    Ok(decoded)
}

pub(super) fn read_with(
    directory: &Path,
    file: &FileRef,
    generation: u64,
    records: u64,
    mut body: impl FnMut(&Segment, &mut CheckedRead) -> io::Result<()>,
) -> io::Result<Vec<Segment>> {
    if generation == 0
        || file.count > records
        || file
            .count
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .is_none_or(|minimum| file.bytes < minimum)
        || file
            .count
            .checked_mul(DESCRIPTOR_BYTES)
            .and_then(|n| n.checked_add(2))
            .is_none_or(|maximum| file.bytes > maximum)
    {
        return Err(invalid("epoch record registry count or byte shape"));
    }
    let mut reader = CheckedRead::open(directory, &file.file, file.bytes, file.blake3)?;
    let budget = Cell::new(DESCRIPTOR_BYTES);
    let mut decoder = serde_json::Deserializer::from_reader(Budget {
        input: &mut reader,
        remaining: &budget,
        reason: "epoch record descriptor exceeds fixed shape",
    });
    let segments = Registry {
        budget: &budget,
        generation,
        segments: file.count,
        records,
    }
    .deserialize(&mut decoder)
    .map_err(io::Error::other)?;
    budget.set(DESCRIPTOR_BYTES);
    decoder.end().map_err(io::Error::other)?;
    drop(decoder);
    reader.finish()?;
    for segment in &segments {
        let mut reader = CheckedRead::open(
            directory,
            &segment.file,
            segment.bytes,
            digest(&segment.blake3)?,
        )?;
        body(segment, &mut reader)?;
        reader.finish()?;
    }
    Ok(segments)
}

#[cfg(test)]
fn read(
    directory: &Path,
    file: &FileRef,
    generation: u64,
    records: u64,
) -> io::Result<Vec<Segment>> {
    use std::io::Read;
    read_with(directory, file, generation, records, |_, reader| {
        let mut scratch = [0; 32 * 1024];
        while reader.read(&mut scratch)? != 0 {}
        Ok(())
    })
}

#[cfg(test)]
mod tests;
