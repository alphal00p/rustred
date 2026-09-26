//! Section codecs. Every binary section starts with a 32-byte little-endian
//! header (magic, tag, arity, flags, semantics version, count, first); fixed
//! width payloads are validated by byte length before any decoding. Digests
//! are computed while writing through `HashingWriter`; nothing is re-read.
use super::super::{
    delegation::{LedgerRef, StoredLedger},
    descendant_closure::{Counters as ClosureCounters, Tracker},
    diagnostics::{OptionalCounts, OptionalRefusals},
    execution::streams::Streams,
    physical_parts::Progress as PhysicalProgress,
    queue::{Domain, QueueMetadata, SortedBuckets, StoredBuckets},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, Write};
use std::sync::Arc;

pub(super) const MAGIC: [u8; 4] = *b"RRW5";
pub(super) const HEADER_BYTES: usize = 32;
pub(super) const FLAG_READY: u16 = 1;
const EDGE_BYTES: usize = 8;
/// Upper bound on one bincode section (ledger, index) and one domain segment.
const MAX_BINCODE_BYTES: usize = 1 << 40;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tag {
    Domains,
    Edges,
    Nodes,
    Ledger,
    Index,
}
impl Tag {
    pub fn bytes(self) -> [u8; 4] {
        match self {
            Self::Domains => *b"DOMS",
            Self::Edges => *b"EDGE",
            Self::Nodes => *b"NODE",
            Self::Ledger => *b"LEDG",
            Self::Index => *b"INDX",
        }
    }
    fn parse(bytes: [u8; 4]) -> Option<Self> {
        [
            Self::Domains,
            Self::Edges,
            Self::Nodes,
            Self::Ledger,
            Self::Index,
        ]
        .into_iter()
        .find(|tag| tag.bytes() == bytes)
    }
}

/// Values every section header of one generation must agree on.
#[derive(Clone, Copy, Debug)]
pub(super) struct Identity {
    pub arity: usize,
    pub ready: bool,
    pub semantics: u32,
}
impl Identity {
    fn flags(&self) -> u16 {
        if self.ready { FLAG_READY } else { 0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Header {
    pub tag: Tag,
    pub arity: u16,
    pub flags: u16,
    pub semantics: u32,
    pub count: u64,
    pub first: u64,
}
impl Header {
    pub fn new(tag: Tag, identity: &Identity, count: usize, first: usize) -> Result<Self, String> {
        Ok(Self {
            tag,
            arity: u16::try_from(identity.arity).map_err(|_| "checkpoint arity exceeds u16")?,
            flags: identity.flags(),
            semantics: identity.semantics,
            count: count as u64,
            first: first as u64,
        })
    }
    pub fn encode(&self) -> [u8; HEADER_BYTES] {
        let mut out = [0u8; HEADER_BYTES];
        out[0..4].copy_from_slice(&MAGIC);
        out[4..8].copy_from_slice(&self.tag.bytes());
        out[8..10].copy_from_slice(&self.arity.to_le_bytes());
        out[10..12].copy_from_slice(&self.flags.to_le_bytes());
        out[12..16].copy_from_slice(&self.semantics.to_le_bytes());
        out[16..24].copy_from_slice(&self.count.to_le_bytes());
        out[24..32].copy_from_slice(&self.first.to_le_bytes());
        out
    }
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let Some(head) = bytes.get(..HEADER_BYTES) else {
            return Err("checkpoint section is shorter than its header".into());
        };
        if head[0..4] != MAGIC {
            return Err("checkpoint section magic mismatch".into());
        }
        let tag = Tag::parse(head[4..8].try_into().expect("four bytes"))
            .ok_or("checkpoint section tag mismatch")?;
        Ok(Self {
            tag,
            arity: u16::from_le_bytes(head[8..10].try_into().expect("two bytes")),
            flags: u16::from_le_bytes(head[10..12].try_into().expect("two bytes")),
            semantics: u32::from_le_bytes(head[12..16].try_into().expect("four bytes")),
            count: u64::from_le_bytes(head[16..24].try_into().expect("eight bytes")),
            first: u64::from_le_bytes(head[24..32].try_into().expect("eight bytes")),
        })
    }
    /// Header agreement with the manifest is validated before any payload.
    pub fn expect(
        &self,
        tag: Tag,
        identity: &Identity,
        count: Option<usize>,
        first: Option<usize>,
    ) -> Result<usize, String> {
        if self.tag != tag {
            return Err("checkpoint section tag mismatch".into());
        }
        if usize::from(self.arity) != identity.arity
            || self.flags != identity.flags()
            || self.semantics != identity.semantics
        {
            return Err("checkpoint section header disagrees with the manifest".into());
        }
        let parsed = usize::try_from(self.count).map_err(|_| "checkpoint section count")?;
        let parsed_first = usize::try_from(self.first).map_err(|_| "checkpoint section first")?;
        if count.is_some_and(|c| c != parsed) || first.is_some_and(|f| f != parsed_first) {
            return Err("checkpoint section range disagrees with the manifest".into());
        }
        Ok(parsed)
    }
}

/// Hashes exactly the bytes that reached the inner writer.
pub(super) struct HashingWriter<W: Write> {
    inner: W,
    hasher: blake3::Hasher,
    bytes: u64,
}
impl<W: Write> HashingWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hasher: blake3::Hasher::new(),
            bytes: 0,
        }
    }
    pub fn finish(mut self) -> io::Result<(u64, String)> {
        self.inner.flush()?;
        Ok((self.bytes, self.hasher.finalize().to_hex().to_string()))
    }
}
impl<W: Write> Write for HashingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn io_error(e: impl std::fmt::Display) -> String {
    format!("checkpoint section write failed: {e}")
}
fn bincode_config() -> impl bincode::config::Config {
    bincode::config::standard().with_limit::<MAX_BINCODE_BYTES>()
}
fn fixed_payload<'a>(
    bytes: &'a [u8],
    tag: Tag,
    identity: &Identity,
    record_bytes: usize,
    count: Option<usize>,
    first: Option<usize>,
) -> Result<(usize, &'a [u8]), String> {
    let header = Header::parse(bytes)?;
    let count = header.expect(tag, identity, count, first)?;
    let expected = count
        .checked_mul(record_bytes)
        .and_then(|n| n.checked_add(HEADER_BYTES))
        .ok_or("checkpoint section length overflow")?;
    if bytes.len() != expected {
        return Err("checkpoint section length disagrees with its record count".into());
    }
    Ok((count, &bytes[HEADER_BYTES..]))
}

// ---- nodes: one flag byte per node --------------------------------------
pub(super) fn write_nodes(
    out: &mut (impl Write + ?Sized),
    identity: &Identity,
    tracker: &Tracker,
) -> Result<(), String> {
    out.write_all(&Header::new(Tag::Nodes, identity, tracker.node_count(), 0)?.encode())
        .map_err(io_error)?;
    let mut buffer = Vec::with_capacity(65536);
    for flag in tracker.node_flags() {
        buffer.push(flag);
        if buffer.len() == buffer.capacity() {
            out.write_all(&buffer).map_err(io_error)?;
            buffer.clear();
        }
    }
    out.write_all(&buffer).map_err(io_error)
}
pub(super) fn read_nodes(bytes: &[u8], identity: &Identity) -> Result<Vec<u8>, String> {
    let (_, payload) = fixed_payload(bytes, Tag::Nodes, identity, 1, None, Some(0))?;
    let mut flags = Vec::new();
    flags
        .try_reserve_exact(payload.len())
        .map_err(|_| "dependency node allocation")?;
    flags.extend_from_slice(payload);
    Ok(flags)
}

// ---- edges: (u32 source, u32 target) pairs in insertion order -------------
pub(super) fn write_edges(
    out: &mut (impl Write + ?Sized),
    identity: &Identity,
    tracker: &Tracker,
    first: usize,
    count: usize,
) -> Result<(), String> {
    out.write_all(&Header::new(Tag::Edges, identity, count, first)?.encode())
        .map_err(io_error)?;
    let mut buffer = Vec::with_capacity(65536);
    let mut written = 0usize;
    for (source, target) in tracker.dependencies().skip(first).take(count) {
        let source = u32::try_from(source).map_err(|_| "dependency endpoint exceeds u32")?;
        let target = u32::try_from(target).map_err(|_| "dependency endpoint exceeds u32")?;
        buffer.extend_from_slice(&source.to_le_bytes());
        buffer.extend_from_slice(&target.to_le_bytes());
        written += 1;
        if buffer.len() + EDGE_BYTES > buffer.capacity() {
            out.write_all(&buffer).map_err(io_error)?;
            buffer.clear();
        }
    }
    if written != count {
        return Err("dependency edge segment shorter than planned".into());
    }
    out.write_all(&buffer).map_err(io_error)
}
pub(super) fn read_edges(
    bytes: &[u8],
    identity: &Identity,
    first: usize,
    count: usize,
    out: &mut Vec<(usize, usize)>,
) -> Result<(), String> {
    let (count, payload) = fixed_payload(
        bytes,
        Tag::Edges,
        identity,
        EDGE_BYTES,
        Some(count),
        Some(first),
    )?;
    out.try_reserve(count)
        .map_err(|_| "dependency edge allocation")?;
    for pair in payload.chunks_exact(EDGE_BYTES) {
        let source = u32::from_le_bytes(pair[0..4].try_into().expect("four bytes"));
        let target = u32::from_le_bytes(pair[4..8].try_into().expect("four bytes"));
        out.push((source as usize, target as usize));
    }
    Ok(())
}

// ---- domains: consecutive bincode records -------------------------------
pub(super) fn write_domains<const N: usize>(
    out: &mut (impl Write + ?Sized),
    identity: &Identity,
    domains: &[Arc<Domain<N>>],
    first: usize,
) -> Result<(), String> {
    let count = domains.len().saturating_sub(first);
    out.write_all(&Header::new(Tag::Domains, identity, count, first)?.encode())
        .map_err(io_error)?;
    let mut out = io::BufWriter::with_capacity(65536, out);
    for domain in &domains[first..] {
        bincode::serde::encode_into_std_write(domain.as_ref(), &mut out, bincode_config())
            .map_err(io_error)?;
    }
    out.flush().map_err(io_error)
}
pub(super) fn read_domains<const N: usize>(
    bytes: &[u8],
    identity: &Identity,
    first: usize,
    count: usize,
    out: &mut Vec<Domain<N>>,
) -> Result<(), String> {
    let header = Header::parse(bytes)?;
    let count = header.expect(Tag::Domains, identity, Some(count), Some(first))?;
    let mut offset = HEADER_BYTES;
    out.try_reserve(count).map_err(|_| "domain allocation")?;
    for _ in 0..count {
        let (domain, used): (Domain<N>, usize) =
            bincode::serde::decode_from_slice(&bytes[offset..], bincode_config())
                .map_err(|e| format!("invalid checkpoint domain record: {e}"))?;
        offset += used;
        out.push(domain);
    }
    if offset != bytes.len() {
        return Err("checkpoint domain segment has trailing bytes".into());
    }
    Ok(())
}

// ---- ledger: bincode of the stored ledger image --------------------------
pub(super) fn write_ledger<K>(
    out: &mut (impl Write + ?Sized),
    identity: &Identity,
    ledger: LedgerRef<'_, K>,
    entries: usize,
) -> Result<(), String> {
    out.write_all(&Header::new(Tag::Ledger, identity, entries, 0)?.encode())
        .map_err(io_error)?;
    let mut out = io::BufWriter::with_capacity(65536, out);
    bincode::serde::encode_into_std_write(&ledger, &mut out, bincode_config()).map_err(io_error)?;
    out.flush().map_err(io_error)
}
pub(super) fn read_ledger(bytes: &[u8], identity: &Identity) -> Result<StoredLedger, String> {
    let header = Header::parse(bytes)?;
    let count = header.expect(Tag::Ledger, identity, None, Some(0))?;
    let (ledger, used): (StoredLedger, usize) =
        bincode::serde::decode_from_slice(&bytes[HEADER_BYTES..], bincode_config())
            .map_err(|e| format!("invalid checkpoint ledger: {e}"))?;
    if used != bytes.len() - HEADER_BYTES || ledger.len() != count {
        return Err("checkpoint ledger length disagrees with its header".into());
    }
    Ok(ledger)
}

// ---- index: bincode of the owner buckets sorted by (phase, owner) --------
pub(super) fn write_index<const N: usize>(
    out: &mut (impl Write + ?Sized),
    identity: &Identity,
    buckets: &SortedBuckets<'_, N>,
) -> Result<(), String> {
    out.write_all(&Header::new(Tag::Index, identity, buckets.len(), 0)?.encode())
        .map_err(io_error)?;
    let mut out = io::BufWriter::with_capacity(65536, out);
    bincode::serde::encode_into_std_write(buckets, &mut out, bincode_config()).map_err(io_error)?;
    out.flush().map_err(io_error)
}
pub(super) fn read_index(bytes: &[u8], identity: &Identity) -> Result<StoredBuckets, String> {
    let header = Header::parse(bytes)?;
    let count = header.expect(Tag::Index, identity, None, Some(0))?;
    let (buckets, used): (StoredBuckets, usize) =
        bincode::serde::decode_from_slice(&bytes[HEADER_BYTES..], bincode_config())
            .map_err(|e| format!("invalid checkpoint index: {e}"))?;
    if used != bytes.len() - HEADER_BYTES || buckets.len() != count {
        return Err("checkpoint index length disagrees with its header".into());
    }
    Ok(buckets)
}

// ---- records: one JSON object per line ----------------------------------
pub(super) fn write_records(
    out: &mut (impl Write + ?Sized),
    records: &[Value],
) -> Result<(), String> {
    let mut out = io::BufWriter::with_capacity(65536, out);
    for record in records {
        serde_json::to_writer(&mut out, record).map_err(io_error)?;
        out.write_all(b"\n").map_err(io_error)?;
    }
    out.flush().map_err(io_error)
}
pub(super) fn read_records(bytes: &[u8], count: usize, out: &mut Vec<Value>) -> Result<(), String> {
    out.try_reserve(count).map_err(|_| "record allocation")?;
    let mut seen = 0usize;
    for line in bytes.split(|&b| b == b'\n') {
        if line.is_empty() {
            continue;
        }
        let record: Value = serde_json::from_slice(line)
            .map_err(|e| format!("invalid checkpoint record line: {e}"))?;
        out.push(record);
        seen += 1;
    }
    if seen != count || (count > 0 && bytes.last() != Some(&b'\n')) {
        return Err("checkpoint record segment count disagrees with the manifest".into());
    }
    Ok(())
}

// ---- meta: everything small and mutable, as JSON -------------------------
#[derive(Serialize)]
pub(super) struct ProgressRef<'a> {
    #[serde(flatten)]
    pub metadata: Value,
    pub physical_parent: &'a Option<PhysicalProgress>,
}
#[derive(Serialize)]
pub(super) struct MetaRef<'a> {
    pub counters: [usize; 12],
    pub route_joint_support_masks_pruned: usize,
    pub queue: QueueMetadata,
    pub closure: ClosureCounters,
    pub details: &'a [Value],
    pub refusals: &'a OptionalRefusals,
    pub optional: &'a OptionalCounts,
    pub progress: ProgressRef<'a>,
    pub parallel: &'a Value,
    pub uncommitted: &'a [Value],
    pub inputs: &'a [Value],
    pub input_frontiers: &'a [Value],
    pub streams: &'a Streams,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Meta {
    pub counters: [usize; 12],
    pub route_joint_support_masks_pruned: usize,
    pub queue: QueueMetadata,
    pub closure: ClosureCounters,
    pub details: Vec<Value>,
    pub refusals: OptionalRefusals,
    pub optional: OptionalCounts,
    pub progress: Value,
    pub parallel: Value,
    pub uncommitted: Vec<Value>,
    pub inputs: Vec<Value>,
    pub input_frontiers: Vec<Value>,
    pub streams: Streams,
}
pub(super) fn write_meta(
    out: &mut (impl Write + ?Sized),
    meta: &MetaRef<'_>,
) -> Result<(), String> {
    let mut out = io::BufWriter::with_capacity(65536, out);
    serde_json::to_writer(&mut out, meta).map_err(io_error)?;
    out.flush().map_err(io_error)
}
pub(super) fn read_meta(bytes: &[u8]) -> Result<Meta, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("invalid checkpoint meta section: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn identity() -> Identity {
        Identity {
            arity: 3,
            ready: true,
            semantics: 7,
        }
    }
    #[test]
    fn header_round_trips_and_validates_identity() {
        let header = Header::new(Tag::Edges, &identity(), 5, 2).unwrap();
        let bytes = header.encode();
        assert_eq!(bytes.len(), HEADER_BYTES);
        assert_eq!(&bytes[..4], b"RRW5");
        assert_eq!(&bytes[4..8], b"EDGE");
        assert_eq!(Header::parse(&bytes).unwrap(), header);
        assert_eq!(
            header.expect(Tag::Edges, &identity(), Some(5), Some(2)),
            Ok(5)
        );
        assert!(header.expect(Tag::Nodes, &identity(), None, None).is_err());
        assert!(
            header
                .expect(Tag::Edges, &identity(), Some(4), None)
                .is_err()
        );
        let mut other = identity();
        other.semantics = 8;
        assert!(header.expect(Tag::Edges, &other, None, None).is_err());
        other = identity();
        other.ready = false;
        assert!(header.expect(Tag::Edges, &other, None, None).is_err());
        let mut corrupt = bytes;
        corrupt[0] = b'X';
        assert!(Header::parse(&corrupt).is_err());
        assert!(Header::parse(&bytes[..31]).is_err());
    }
    #[test]
    fn fixed_width_sections_reject_length_mismatch_before_decoding() {
        let mut tracker = Tracker::new(2);
        tracker.discovered(3);
        tracker.edge(0, 2);
        tracker.edge(1, 2);
        let mut bytes = Vec::new();
        write_edges(&mut bytes, &identity(), &tracker, 0, 2).unwrap();
        assert_eq!(bytes.len(), HEADER_BYTES + 2 * EDGE_BYTES);
        let mut edges = Vec::new();
        read_edges(&bytes, &identity(), 0, 2, &mut edges).unwrap();
        assert_eq!(edges, [(0, 2), (1, 2)]);
        let truncated = &bytes[..bytes.len() - 1];
        assert!(
            read_edges(truncated, &identity(), 0, 2, &mut Vec::new())
                .unwrap_err()
                .contains("length")
        );
        let mut extended = bytes.clone();
        extended.push(0);
        assert!(read_edges(&extended, &identity(), 0, 2, &mut Vec::new()).is_err());
        let mut nodes = Vec::new();
        write_nodes(&mut nodes, &identity(), &tracker).unwrap();
        assert_eq!(read_nodes(&nodes, &identity()).unwrap(), [0, 0, 0]);
        nodes.pop();
        assert!(read_nodes(&nodes, &identity()).is_err());
        let mut hashed = HashingWriter::new(Vec::new());
        hashed.write_all(b"abc").unwrap();
        hashed.write_all(b"def").unwrap();
        let (count, digest) = hashed.finish().unwrap();
        assert_eq!(count, 6);
        assert_eq!(digest, blake3::hash(b"abcdef").to_hex().to_string());
    }
    #[test]
    fn record_lines_must_match_the_declared_count() {
        let records = [
            serde_json::json!({"id":0}),
            serde_json::json!({"id":1,"x":"a\nb"}),
        ];
        let mut bytes = Vec::new();
        write_records(&mut bytes, &records).unwrap();
        let mut out = Vec::new();
        read_records(&bytes, 2, &mut out).unwrap();
        assert_eq!(out, records);
        assert!(read_records(&bytes, 1, &mut Vec::new()).is_err());
        assert!(read_records(&bytes[..bytes.len() - 1], 2, &mut Vec::new()).is_err());
        read_records(b"", 0, &mut Vec::new()).unwrap();
    }
}
