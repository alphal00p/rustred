//! Provisional fixed-width fields/run-log assembly, not a restore entry point.
//! Counts must agree with actual file lengths before any count-sized reserve.
//! Arrays are the final runtime representation, never a full encoded copy.
//! Cross-section geometry, records, roots, anchors and dispatch/session checks
//! must still pass before constructing a usable EpochState or running workers.
use super::super::super::queue::Query;
use super::super::edges::EdgeStore;
use super::super::job::{Reader, read_image};
use super::super::ledger6::{Entry6, Ledger6};
use super::super::state::{NODE_ANCHORED, NODE_INSPECTED, NODE_SEALED};
use super::super::store::Store;
use super::super::verify::QueryImage;
use super::read::CheckedRead;
use super::{Section, SectionReceipt, invalid};
use std::io::{self, Read};
use std::path::Path;

pub(super) struct FixedSection<const N: usize> {
    reader: CheckedRead,
    section: Section,
    count: usize,
}

impl<const N: usize> FixedSection<N> {
    pub fn open(directory: &Path, receipt: &SectionReceipt, count: u64) -> io::Result<Self> {
        if N > 32 || receipt.generation == 0 {
            return Err(invalid("invalid epoch section arity or generation"));
        }
        let mut reader = CheckedRead::open(
            directory,
            &receipt.section.filename(receipt.generation),
            receipt.digest.bytes,
            receipt.digest.blake3,
        )?;
        let mut magic = [0; 8];
        reader.read_exact(&mut magic)?;
        if &magic != b"EPC6PART"
            || reader.u32()? != 1
            || reader.u32()? != N as u32
            || reader.u32()? != receipt.section as u32
            || reader.u64()? != count
        {
            return Err(invalid("epoch section header differs"));
        }
        let width = match receipt.section {
            Section::Domains => 37 + 4 * N as u64,
            Section::Nodes | Section::ClosureFlags => 1,
            Section::Live | Section::Ledger | Section::Frontiers => 8,
            // Edge headers count runs, not u32 words. A run has at least
            // source/count (8 bytes), followed by its variable target list.
            Section::Edges => {
                if reader.remaining() % 4 != 0
                    || count
                        .checked_mul(8)
                        .is_none_or(|minimum| minimum > reader.remaining())
                    || (count == 0) != (reader.remaining() == 0)
                {
                    return Err(invalid("epoch edge run count does not fit byte length"));
                }
                0
            }
            Section::Anchors | Section::Dispatch => {
                return Err(invalid("variable epoch section needs its own decoder"));
            }
        };
        if width != 0 && count.checked_mul(width) != Some(reader.remaining()) {
            return Err(invalid("epoch section count does not match byte length"));
        }
        let count = usize::try_from(count).map_err(|_| invalid("epoch section count range"))?;
        if matches!(
            receipt.section,
            Section::Domains | Section::Nodes | Section::Ledger | Section::ClosureFlags
        ) && count >= u32::MAX as usize
        {
            return Err(invalid("epoch domain watermark range"));
        }
        Ok(Self {
            reader,
            section: receipt.section,
            count,
        })
    }

    fn expect(&self, section: Section) -> io::Result<()> {
        if self.section != section {
            return Err(invalid("wrong epoch section decoder"));
        }
        Ok(())
    }

    /// Canonical arena, summaries, exact index and bucket interning only.
    /// No lookup index or dominant orthant is rebuilt here: persisted live
    /// membership and historical orthants have different restore rules.
    pub fn domains(mut self) -> io::Result<Store<N>> {
        self.expect(Section::Domains)?;
        let mut store = Store::new();
        let mut bytes = [0u8; 37 + 4 * 32];
        for _ in 0..self.count {
            let bytes = &mut bytes[..37 + 4 * N];
            self.reader.read_exact(bytes)?;
            let mut reader = Reader::new(bytes);
            let image = read_image::<N>(&mut reader).map_err(invalid)?;
            reader.finish().map_err(invalid)?;
            let query = QueryImage::new(image).map_err(invalid)?;
            let compact = Query::new(query.core, image.phase()).compact;
            store.try_reserve(1).map_err(io::Error::other)?;
            store
                .exact
                .try_reserve_one(query.digest)
                .map_err(io::Error::other)?;
            store
                .push(image, compact, query.digest)
                .map_err(io::Error::other)?;
        }
        self.reader.finish()?;
        Ok(store)
    }

    pub fn flags(mut self, closure: bool) -> io::Result<Vec<u8>> {
        self.expect(if closure {
            Section::ClosureFlags
        } else {
            Section::Nodes
        })?;
        let allowed = if closure {
            7
        } else {
            NODE_SEALED | NODE_INSPECTED | NODE_ANCHORED
        };
        let mut flags = Vec::new();
        flags
            .try_reserve_exact(self.count)
            .map_err(|_| io::Error::other("epoch flags allocation"))?;
        for _ in 0..self.count {
            let flag = self.reader.u8()?;
            if flag & !allowed != 0 {
                return Err(invalid("unsupported epoch node or closure flag"));
            }
            flags.push(flag);
        }
        self.reader.finish()?;
        Ok(flags)
    }

    pub fn live(mut self, watermark: u32) -> io::Result<Vec<u64>> {
        self.expect(Section::Live)?;
        if watermark == u32::MAX || self.count != (watermark as usize).div_ceil(64) {
            return Err(invalid("epoch live word count differs"));
        }
        let mut words = Vec::new();
        words
            .try_reserve_exact(self.count)
            .map_err(|_| io::Error::other("epoch live allocation"))?;
        for _ in 0..self.count {
            words.push(self.reader.u64()?);
        }
        if watermark % 64 != 0
            && words
                .last()
                .is_some_and(|word| word >> (watermark % 64) != 0)
        {
            return Err(invalid("epoch live padding bits"));
        }
        self.reader.finish()?;
        Ok(words)
    }

    pub fn ledger(mut self) -> io::Result<Ledger6> {
        self.expect(Section::Ledger)?;
        let mut ledger = Ledger6::default();
        ledger.try_reserve(self.count).map_err(io::Error::other)?;
        for id in 0..self.count {
            let word = self.reader.u64()?;
            match Entry6::decode(word).map_err(|error| io::Error::other(error.to_string()))? {
                Entry6::Native { residual: true, .. } => {
                    return Err(invalid("unsupported epoch residual ledger"));
                }
                Entry6::Alias { to } if to as usize <= id || to as usize >= self.count => {
                    return Err(invalid("epoch alias target range or order"));
                }
                _ => {}
            }
            ledger
                .restore_word(word)
                .map_err(|error| io::Error::other(error.to_string()))?;
        }
        self.reader.finish()?;
        Ok(ledger)
    }

    pub fn edges(mut self, watermark: u32) -> io::Result<EdgeStore> {
        self.expect(Section::Edges)?;
        if watermark == u32::MAX {
            return Err(invalid("epoch edge watermark range"));
        }
        let word_count = usize::try_from(self.reader.remaining() / 4)
            .map_err(|_| invalid("epoch edge word count range"))?;
        let mut words = Vec::new();
        words
            .try_reserve_exact(word_count)
            .map_err(|_| io::Error::other("epoch edge log allocation"))?;
        for _ in 0..word_count {
            words.push(self.reader.u32()?);
        }
        self.reader.finish()?;
        let edges = EdgeStore::from_owned_log(words, watermark).map_err(io::Error::other)?;
        if edges.runs() != self.count as u64 {
            return Err(invalid("epoch decoded edge run count differs"));
        }
        Ok(edges)
    }
}

#[cfg(test)]
mod tests;
