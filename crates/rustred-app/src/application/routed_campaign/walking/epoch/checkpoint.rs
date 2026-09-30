//! CP6 S3 checkpoint implementation for the public durable lockstep lifecycle.
//!
//! Full restore validation precedes session reservation and dispatch. Native
//! and outer-adapter execution gates still precede any deployment claim.
//! The writer borrows one state for its entire lifetime, preserves live bits
//! and exact dispatch order, and uses fixed scratch independent of domains.
#![allow(dead_code)] // Private lifecycle lands before its runtime integration.

use super::anchors::{ANCHORS_VERSION, MAX_ANCHORS, scope_bytes};
use super::dispatch::{Dispatch, DispatchSnapshot};
use super::job::{Writer, write_image};
use super::ledger6::{EPOCH_LIMIT, Entry6, Tag};
use super::state::EpochState;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::Path;

mod metadata;
mod publication;
pub(crate) use publication::{
    FORMAT as EPOCH_WALK_CHECKPOINT_FORMAT, SCHEMA as EPOCH_WALK_CHECKPOINT_SCHEMA,
};
mod read;
mod restore;
pub(super) use restore::run;
mod session;
mod stop;
pub(super) use session::Session;
#[cfg(test)]
mod tests;

const BUFFER_BYTES: usize = 32 * 1024;
const MEMBERSHIP_WORDS: usize = 1024;

/// No auxiliary thread or arena scan: notify after actual streamed writes.
/// The caller rate-limits observer transport independently of write frequency.
pub(super) struct Observed<'a, W> {
    pub output: W,
    pub progress: &'a mut dyn FnMut(),
}
impl<W: Write> Write for Observed<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.output.write(bytes)?;
        (self.progress)();
        Ok(written)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.output.flush()?;
        (self.progress)();
        Ok(())
    }
}
pub(super) const SEQUENCE_COUNTER_LIMIT: u64 = 1 << 40;
pub(super) const SESSION_LIMIT: u64 = 1 << 24;

fn invalid(reason: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, reason)
}

/// An immutable state-bound save borrow. Construction is private to this
/// module; callers cannot pair a token with a different or mutated state.
/// These checks are structural, not the later full restore/certification
/// validators. In particular this is not proof that all initial queries
/// were admitted; only the eventual bound input inventory can prove that.
pub(super) struct MergeBoundary<'a, const N: usize> {
    state: &'a EpochState<N>,
    dispatch: DispatchSnapshot<'a>,
    lockstep_b: usize,
}

impl<'a, const N: usize> MergeBoundary<'a, N> {
    pub fn borrow(
        state: &'a EpochState<N>,
        dispatch: &'a Dispatch,
        lockstep_b: usize,
    ) -> io::Result<Self> {
        let dispatch = dispatch.checkpoint_snapshot();
        let count = state.store.len();
        if state.poisoned {
            return Err(invalid("cannot save a poisoned epoch merge"));
        }
        if N > 32
            || count >= u32::MAX as usize
            || state.ledger.words().len() != count
            || state.nodes.len() != count
            || state.live.len() != count.div_ceil(64)
            || state.p0 as usize > count
            || state.k >= EPOCH_LIMIT
            || dispatch.cursor as usize > count
            || !(1..=4096).contains(&lockstep_b)
            || state.in_flight.len() > lockstep_b
        {
            return Err(invalid("epoch boundary shape or lockstep bound"));
        }
        if count % 64 != 0
            && state
                .live
                .last()
                .is_some_and(|word| word >> (count % 64) != 0)
        {
            return Err(invalid("epoch live bits beyond the watermark"));
        }
        if dispatch.session == 0
            || dispatch.session >= SESSION_LIMIT
            || dispatch.counter >= SEQUENCE_COUNTER_LIMIT
        {
            return Err(invalid("epoch dispatch sequence range"));
        }
        if state.counters.g2_records != state.anchors.records().iter().filter(|r| r.kind.is_g2()).count() as u64
            || (0..state.watermark()).any(|id| {
                let residual = state.anchors.get(id).is_some_and(|r| r.kind.is_g2());
                (state.nodes[id as usize] & super::state::NODE_RESIDUAL != 0) != residual
                    || matches!(state.ledger.get(id), Ok(Entry6::Native { residual: bit, .. }) if bit != residual)
            }) {
            return Err(invalid("epoch residual anchor, ledger and counter inventory differs"));
        }
        let boundary = Self {
            state,
            dispatch,
            lockstep_b,
        };
        Reservations {
            ledger: &state.ledger,
            nodes: &state.nodes,
            k: state.k,
            in_flight: &state.in_flight,
            dispatch: boundary.dispatch,
            lockstep_b,
        }
        .validate()?;
        Ok(boundary)
    }

    /// Write one immutable *unpublished* section; an I/O failure leaves an
    /// orphan, never a success receipt. Existing files are never overwritten.
    /// Directory durability belongs to the later manifest publisher.
    pub fn write_new_section(
        &self,
        directory: &Path,
        generation: u64,
        section: Section,
    ) -> io::Result<SectionReceipt> {
        self.write_new_section_observed(directory, generation, section, &mut || {})
    }

    pub(super) fn write_new_section_observed(
        &self,
        directory: &Path,
        generation: u64,
        section: Section,
        progress: &mut dyn FnMut(),
    ) -> io::Result<SectionReceipt> {
        if generation == 0 {
            return Err(invalid("epoch generation zero"));
        }
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(directory.join(section.filename(generation)))?;
        let (file, digest) = self.write_section(
            Observed {
                output: file,
                progress,
            },
            section,
        )?;
        file.output.sync_all()?;
        Ok(SectionReceipt {
            generation,
            section,
            digest,
        })
    }

    fn write_section<W: Write>(&self, output: W, section: Section) -> io::Result<(W, Digest)> {
        let state = self.state;
        let mut out = Stream::new(output);
        out.write_all(b"EPC6PART")?;
        out.write_all(&1u32.to_le_bytes())?;
        out.write_all(&(N as u32).to_le_bytes())?;
        out.write_all(&(section as u32).to_le_bytes())?;
        out.write_all(&self.section_count(section).to_le_bytes())?;
        match section {
            Section::Domains => {
                // At most 165 bytes (N <= 32); reused for every image.
                let mut image = Writer(Vec::new());
                image
                    .0
                    .try_reserve_exact(37 + 4 * N)
                    .map_err(|_| io::Error::other("epoch image scratch allocation"))?;
                for domain in &state.store.domains {
                    image.0.clear();
                    write_image(&mut image, domain);
                    out.write_all(&image.0)?;
                }
            }
            Section::Nodes => out.write_all(&state.nodes)?,
            Section::Live | Section::Ledger => {
                let words = if section == Section::Live {
                    &state.live[..]
                } else {
                    state.ledger.words()
                };
                for word in words {
                    out.write_all(&word.to_le_bytes())?;
                }
            }
            Section::Edges => {
                for word in state.edges.log() {
                    out.write_all(&word.to_le_bytes())?;
                }
            }
            Section::Anchors => self.write_anchors(&mut out)?,
            Section::Dispatch => {
                for value in [
                    state.k,
                    u64::from(state.p0),
                    self.lockstep_b as u64,
                    self.dispatch.session,
                    self.dispatch.counter,
                    u64::from(self.dispatch.cursor),
                ] {
                    out.write_all(&value.to_le_bytes())?;
                }
                for queue in [self.dispatch.requeue, self.dispatch.deferred] {
                    out.write_all(&(queue.len() as u64).to_le_bytes())?;
                    for id in queue {
                        out.write_all(&id.to_le_bytes())?;
                    }
                }
                out.write_all(&(state.in_flight.len() as u64).to_le_bytes())?;
                for (id, meta) in &state.in_flight {
                    out.write_all(&id.to_le_bytes())?;
                    out.write_all(&meta.seq.to_le_bytes())?;
                    out.write_all(&meta.v0.to_le_bytes())?;
                }
            }
            Section::ClosureFlags => {
                for flags in state.tracker.node_flags() {
                    out.write_all(&[flags])?;
                }
            }
            Section::Frontiers => {
                for (id, count) in &state.frontier_counts {
                    out.write_all(&id.to_le_bytes())?;
                    out.write_all(&count.to_le_bytes())?;
                }
            }
        }
        out.finish()
    }

    fn section_count(&self, section: Section) -> u64 {
        let state = self.state;
        match section {
            Section::Domains | Section::Nodes | Section::Ledger => state.store.len() as u64,
            Section::Live => state.live.len() as u64,
            Section::Edges => state.edges.runs(),
            Section::Anchors => state.anchors.len() as u64,
            Section::Dispatch => {
                (self.dispatch.requeue.len() + self.dispatch.deferred.len() + state.in_flight.len())
                    as u64
            }
            Section::ClosureFlags => state.tracker.node_flags().count() as u64,
            Section::Frontiers => state.frontier_counts.len() as u64,
        }
    }

    fn write_anchors(&self, out: &mut impl Write) -> io::Result<()> {
        out.write_all(&ANCHORS_VERSION.to_le_bytes())?;
        out.write_all(&(self.state.anchors.len() as u64).to_le_bytes())?;
        let mut written = 0;
        // Lookup in node order replaces encode()'s whole-map sorting Vec.
        for id in 0..self.state.watermark() {
            let Some(record) = self.state.anchors.get(id) else {
                continue;
            };
            let scope = scope_bytes(&record.scope).map_err(io::Error::other)?;
            if record.anchors.is_empty() || record.anchors.len() > MAX_ANCHORS {
                return Err(invalid("epoch anchor count outside bound"));
            }
            out.write_all(&record.node.to_le_bytes())?;
            out.write_all(&[record.kind as u8, 0, 0, 0])?;
            out.write_all(&(record.anchors.len() as u32).to_le_bytes())?;
            out.write_all(
                &u32::try_from(scope.len())
                    .map_err(|_| invalid("epoch scope length"))?
                    .to_le_bytes(),
            )?;
            out.write_all(&record.dispatch_version.to_le_bytes())?;
            for anchor in &record.anchors {
                if anchor.stamp == Some(u64::MAX) {
                    return Err(invalid("noncanonical epoch anchor stamp"));
                }
                out.write_all(&anchor.anchor.to_le_bytes())?;
                out.write_all(&[anchor.lent as u8, 0, 0, 0])?;
                out.write_all(&anchor.stamp.unwrap_or(u64::MAX).to_le_bytes())?;
            }
            out.write_all(&scope)?;
            written += 1;
        }
        if written != self.state.anchors.len() {
            return Err(invalid("epoch anchor outside domain watermark"));
        }
        Ok(())
    }
}

/// Borrowed reservation protocol shared by the writer and provisional restore.
/// It neither owns nor clones domain-sized state and cannot issue job sequences.
pub(super) struct Reservations<'a> {
    pub ledger: &'a super::ledger6::Ledger6,
    pub nodes: &'a [u8],
    pub k: u64,
    pub in_flight: &'a std::collections::BTreeMap<u32, super::state::JobMeta>,
    pub dispatch: DispatchSnapshot<'a>,
    pub lockstep_b: usize,
}

impl Reservations<'_> {
    fn queued(&self) -> impl Iterator<Item = u32> + '_ {
        self.dispatch
            .requeue
            .iter()
            .chain(self.dispatch.deferred)
            .copied()
    }

    fn reserved_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.queued().chain(self.in_flight.keys().copied())
    }

    pub fn validate(&self) -> io::Result<()> {
        let state = self;
        if self.nodes.len() != self.ledger.words().len()
            || self.nodes.len() >= u32::MAX as usize
            || self.k >= EPOCH_LIMIT
            || self.dispatch.cursor as usize > self.nodes.len()
            || self.dispatch.session == 0
            || self.dispatch.session >= SESSION_LIMIT
            || self.dispatch.counter >= SEQUENCE_COUNTER_LIMIT
            || !(1..=4096).contains(&self.lockstep_b)
            || self.in_flight.len() > self.lockstep_b
            || self
                .dispatch
                .adaptive
                .is_some_and(|saved| !saved.valid(self.dispatch.cursor))
        {
            return Err(invalid("epoch reservation view shape or range"));
        }
        let mut reserved = 0u64;
        for (id, _) in state.ledger.words().iter().enumerate() {
            let entry = state
                .ledger
                .get(id as u32)
                .map_err(|_| invalid("epoch ledger word"))?;
            if matches!(entry, Entry6::Reserved(_)) {
                reserved += 1;
            }
            if matches!(entry, Entry6::Pending(_))
                && id < self.dispatch.cursor as usize
                && !self
                    .dispatch
                    .adaptive
                    .is_some_and(|saved| saved.candidates.binary_search(&(id as u32)).is_ok())
            {
                return Err(invalid("epoch pending ID behind dispatch cursor"));
            }
        }
        if reserved != state.ledger.counts().get(Tag::Reserved)
            || reserved != self.reserved_ids().count() as u64
        {
            return Err(invalid("epoch reserved/dispatch cardinality differs"));
        }
        for (id, deferred) in self
            .dispatch
            .requeue
            .iter()
            .map(|id| (*id, false))
            .chain(self.dispatch.deferred.iter().map(|id| (*id, true)))
        {
            let Ok(Entry6::Reserved(counters)) = state.ledger.get(id) else {
                return Err(invalid("epoch queued ID is not Reserved"));
            };
            if (counters.attempts >= 2) != deferred {
                return Err(invalid("epoch queued ID in wrong attempts class"));
            }
        }
        for (&id, meta) in state.in_flight {
            if !matches!(state.ledger.get(id), Ok(Entry6::Reserved(_)))
                // Unpublished work is reissued against the saved generation.
                || meta.v0 > state.k
                || meta.seq >> 40 != self.dispatch.session
                || meta.seq & (SEQUENCE_COUNTER_LIMIT - 1) == 0
                || meta.seq & (SEQUENCE_COUNTER_LIMIT - 1) > self.dispatch.counter
                || state
                    .in_flight
                    .range(..id)
                    .any(|(_, earlier)| earlier.seq == meta.seq)
            {
                return Err(invalid("epoch in-flight descriptor or sequence"));
            }
        }
        // Exact disjointness without a domain-sized allocation. Each 64-Ki
        // ID window uses the same 8-KiB bitset; all descriptors were already
        // range/tag checked above. Cardinality then proves completeness.
        let width = (MEMBERSHIP_WORDS * 64) as u64;
        if let (Some(min), Some(max)) = (self.reserved_ids().min(), self.reserved_ids().max()) {
            let mut start = u64::from(min) / width * width;
            while start <= u64::from(max) {
                let mut seen = [0u64; MEMBERSHIP_WORDS];
                let mut next = None::<u64>;
                for id in self.reserved_ids().map(u64::from) {
                    if id >= start && id < start + width {
                        let local = (id - start) as usize;
                        let bit = 1u64 << (local % 64);
                        let word = &mut seen[local / 64];
                        if *word & bit != 0 {
                            return Err(invalid("epoch duplicate queued/in-flight ID"));
                        }
                        *word |= bit;
                    } else if id >= start + width {
                        next = Some(next.map_or(id, |value| value.min(id)));
                    }
                }
                let Some(next) = next else { break };
                start = next / width * width;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Section {
    Domains = 1,
    Nodes = 2,
    Live = 3,
    Ledger = 4,
    Edges = 5,
    Anchors = 6,
    Dispatch = 7,
    ClosureFlags = 8,
    Frontiers = 9,
}

impl Section {
    const ALL: [Self; 9] = [
        Self::Domains,
        Self::Nodes,
        Self::Live,
        Self::Ledger,
        Self::Edges,
        Self::Anchors,
        Self::Dispatch,
        Self::ClosureFlags,
        Self::Frontiers,
    ];
    pub fn filename(self, generation: u64) -> String {
        format!("epoch-{generation:020}-{}.part", self as u32)
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Digest {
    pub bytes: u64,
    pub blake3: [u8; 32],
}

#[derive(Debug)]
pub(super) struct SectionReceipt {
    pub generation: u64,
    pub section: Section,
    pub digest: Digest,
}

/// Fixed-memory buffering and incremental hashing. Failed writes/flushes
/// never return a digest receipt. No destructor retries buffered I/O after
/// failure, so fault ordering is explicit and reproducible.
pub(super) struct Stream<W> {
    output: W,
    buffer: [u8; BUFFER_BYTES],
    used: usize,
    bytes: u64,
    hash: blake3::Hasher,
    failed: bool,
}

impl<W: Write> Stream<W> {
    pub(super) fn new(output: W) -> Self {
        Self {
            output,
            buffer: [0; BUFFER_BYTES],
            used: 0,
            bytes: 0,
            hash: blake3::Hasher::new(),
            failed: false,
        }
    }

    fn drain(&mut self) -> io::Result<()> {
        if self.failed {
            return Err(io::Error::other("epoch section writer already failed"));
        }
        let bytes = self
            .bytes
            .checked_add(self.used as u64)
            .ok_or_else(|| invalid("epoch section byte count overflow"))?;
        if let Err(error) = self.output.write_all(&self.buffer[..self.used]) {
            self.failed = true;
            return Err(error);
        }
        self.hash.update(&self.buffer[..self.used]);
        self.bytes = bytes;
        self.used = 0;
        Ok(())
    }

    pub(super) fn finish(mut self) -> io::Result<(W, Digest)> {
        self.flush()?;
        Ok((
            self.output,
            Digest {
                bytes: self.bytes,
                blake3: *self.hash.finalize().as_bytes(),
            },
        ))
    }
}

impl<W: Write> Write for Stream<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.failed {
            return Err(io::Error::other("epoch section writer already failed"));
        }
        if self.used == BUFFER_BYTES {
            self.drain()?;
        }
        let n = bytes.len().min(BUFFER_BYTES - self.used);
        self.buffer[self.used..self.used + n].copy_from_slice(&bytes[..n]);
        self.used += n;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.drain()?;
        if let Err(error) = self.output.flush() {
            self.failed = true;
            return Err(error);
        }
        Ok(())
    }
}
