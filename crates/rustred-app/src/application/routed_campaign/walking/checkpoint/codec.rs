use super::super::{
    diagnostics::{OptionalCounts, OptionalRefusals},
    queue::Queue,
};
use super::*;
use std::io::{self, Read, Write};

const MAGIC: &[u8] = b"RUSTRED-WALK-CP1\n";
const READY_MAGIC: &[u8] = b"RUSTRED-WALK-CP2\n";
const CHUNK: usize = 65536;
struct FramedWriter<W> {
    inner: W,
    buffer: Vec<u8>,
}
impl<W: Write> FramedWriter<W> {
    fn new(mut inner: W, ready: bool) -> io::Result<Self> {
        inner.write_all(if ready { READY_MAGIC } else { MAGIC })?;
        Ok(Self {
            inner,
            buffer: Vec::with_capacity(CHUNK),
        })
    }
    fn chunk(&mut self) -> io::Result<()> {
        if !self.buffer.is_empty() {
            self.inner
                .write_all(&(self.buffer.len() as u32).to_le_bytes())?;
            self.inner.write_all(&self.buffer)?;
            self.buffer.clear();
        }
        Ok(())
    }
    fn finish(mut self) -> io::Result<()> {
        self.chunk()?;
        self.inner.write_all(&0u32.to_le_bytes())?;
        self.inner.flush()
    }
}
impl<W: Write> Write for FramedWriter<W> {
    fn write(&mut self, mut bytes: &[u8]) -> io::Result<usize> {
        let total = bytes.len();
        while !bytes.is_empty() {
            let n = bytes.len().min(CHUNK - self.buffer.len());
            self.buffer.extend_from_slice(&bytes[..n]);
            bytes = &bytes[n..];
            if self.buffer.len() == CHUNK {
                self.chunk()?;
            }
        }
        Ok(total)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.chunk()?;
        self.inner.flush()
    }
}
struct FramedReader<R> {
    inner: R,
    remaining: usize,
    done: bool,
    ready: bool,
}
impl<R: Read> FramedReader<R> {
    fn new(mut inner: R) -> io::Result<Self> {
        let mut magic = vec![0; MAGIC.len()];
        inner.read_exact(&mut magic)?;
        if magic != MAGIC && magic != READY_MAGIC {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "checkpoint binary format mismatch",
            ));
        }
        Ok(Self {
            inner,
            remaining: 0,
            done: false,
            ready: magic == READY_MAGIC,
        })
    }
}
impl<R: Read> Read for FramedReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        if out.is_empty() || self.done {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut length = [0; 4];
            self.inner.read_exact(&mut length)?;
            self.remaining = u32::from_le_bytes(length) as usize;
            if self.remaining > CHUNK {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "checkpoint frame exceeds bound",
                ));
            }
            if self.remaining == 0 {
                self.done = true;
                let mut extra = [0; 1];
                if self.inner.read(&mut extra)? != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "trailing checkpoint bytes",
                    ));
                }
                return Ok(0);
            }
        }
        let n = out.len().min(self.remaining);
        self.inner.read_exact(&mut out[..n])?;
        self.remaining -= n;
        Ok(n)
    }
}
#[derive(Serialize)]
struct ProgressRef<'a> {
    #[serde(flatten)]
    metadata: Value,
    physical_parent: &'a Option<super::super::physical_parts::Progress>,
}
#[derive(Serialize)]
struct ImageRef<'a, const N: usize> {
    queue: &'a Queue<N>,
    records: &'a [Value],
    details: &'a [Value],
    refusals: &'a OptionalRefusals,
    optional: &'a OptionalCounts,
    counters: [usize; 12],
    progress: ProgressRef<'a>,
    parallel: &'a Value,
    uncommitted: &'a [Value],
    inputs: &'a [Value],
    input_frontiers: &'a [Value],
    streams: &'a super::super::execution::streams::Streams,
}
#[derive(Deserialize)]
struct Image<const N: usize> {
    queue: Queue<N>,
    records: Vec<Value>,
    details: Vec<Value>,
    refusals: OptionalRefusals,
    optional: OptionalCounts,
    counters: [usize; 12],
    progress: Value,
    parallel: Value,
    uncommitted: Vec<Value>,
    inputs: Vec<Value>,
    input_frontiers: Vec<Value>,
    #[serde(default)]
    streams: Option<super::super::execution::streams::Streams>,
}
pub(in super::super) struct Restored<const N: usize> {
    pub state: State<N>,
    pub inputs: Vec<Value>,
    pub input_frontiers: Vec<Value>,
}
pub(in super::super) fn write<const N: usize>(
    out: impl Write,
    s: &State<N>,
    inputs: &[Value],
    input_frontiers: &[Value],
) -> Result<(), String> {
    let mut out = FramedWriter::new(out, s.ready()).map_err(|e| e.to_string())?;
    let image = ImageRef {
        queue: &s.queue,
        records: &s.records,
        details: &s.details,
        refusals: &s.refusals,
        optional: &s.optional,
        counters: [
            s.events,
            s.successors,
            s.conditional,
            s.job_local_reuse_hits,
            s.pre_admitted_orthant_hits,
            s.frontiers,
            s.completed,
            s.native_records,
            s.routed,
            s.route_masks,
            s.initial_domain_count,
            s.initial_entry_domains_inspected,
        ],
        progress: ProgressRef {
            metadata: s.checkpoint_progress_metadata(),
            physical_parent: &s.physical_progress,
        },
        parallel: &s.parallel,
        uncommitted: &s.uncommitted,
        inputs,
        input_frontiers,
        streams: &s.streams,
    };
    serde_json::to_writer(&mut out, &image).map_err(|e| e.to_string())?;
    out.finish().map_err(|e| e.to_string())
}
pub(in super::super) fn read<const N: usize>(input: impl Read) -> Result<Restored<N>, String> {
    let reader = FramedReader::new(input).map_err(|e| e.to_string())?;
    let ready_format = reader.ready;
    let image: Image<N> =
        serde_json::from_reader(reader).map_err(|e| format!("invalid checkpoint state: {e}"))?;
    let [
        events,
        successors,
        conditional,
        job_local_reuse_hits,
        pre_admitted_orthant_hits,
        frontiers,
        completed,
        native_records,
        routed,
        route_masks,
        initial_domain_count,
        initial_entry_domains_inspected,
    ] = image.counters;
    if initial_domain_count > image.queue.domains.len()
        || initial_entry_domains_inspected > initial_domain_count
        || completed > native_records
        || native_records
            > image
                .queue
                .delegation
                .as_ref()
                .map_or(image.queue.next, |l| l.published_count())
        || ready_format
            != image
                .queue
                .delegation
                .as_ref()
                .is_some_and(|l| l.is_ready())
    {
        return Err("inconsistent checkpoint publication counters".into());
    }
    let mut s = State::new(image.queue, frontiers, None);
    s.records = image.records;
    s.details = image.details;
    s.refusals = image.refusals;
    s.optional = image.optional;
    s.events = events;
    s.successors = successors;
    s.conditional = conditional;
    s.job_local_reuse_hits = job_local_reuse_hits;
    s.pre_admitted_orthant_hits = pre_admitted_orthant_hits;
    s.completed = completed;
    s.native_records = native_records;
    s.routed = routed;
    s.route_masks = route_masks;
    s.initial_domain_count = initial_domain_count;
    s.initial_entry_domains_inspected = initial_entry_domains_inspected;
    s.parallel = image.parallel;
    s.uncommitted = image.uncommitted;
    s.restore_checkpoint_progress(image.progress)?;
    s.streams = match image.streams {
        Some(streams) => streams,
        None if !ready_format => Default::default(),
        None => return Err("ready checkpoint has no stream contexts".into()),
    };
    s.validate_restored_streams()?;
    Ok(Restored {
        state: s,
        inputs: image.inputs,
        input_frontiers: image.input_frontiers,
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::{
        delegation::{NativeOutcome, SchedulingPolicy},
        queue::{Domain, Phase},
    };
    use super::*;
    #[test]
    fn bounded_frames_round_trip_and_reject_truncation() {
        let payload = vec![b'x'; CHUNK * 3 + 17];
        let mut bytes = Vec::new();
        let mut w = FramedWriter::new(&mut bytes, false).unwrap();
        w.write_all(&payload).unwrap();
        w.finish().unwrap();
        let mut out = Vec::new();
        FramedReader::new(bytes.as_slice())
            .unwrap()
            .read_to_end(&mut out)
            .unwrap();
        assert_eq!(out, payload);
        bytes.pop();
        assert!(
            FramedReader::new(bytes.as_slice())
                .unwrap()
                .read_to_end(&mut Vec::new())
                .is_err()
        );
    }

    #[test]
    fn round_trip_preserves_retirement_aliases_and_restarts_only_unfinished_native() {
        let mut queue = Queue::with_policy(
            100,
            None,
            SchedulingPolicy::TransferUnreserved {
                lookahead: std::num::NonZeroUsize::new(1).unwrap(),
            },
        )
        .unwrap();
        let domain = |lo, hi| Domain {
            phase: Phase::Apply,
            owner: [true],
            lower: vec![lo],
            upper: vec![Some(hi)],
            rank: None,
            powers: Default::default(),
        };
        queue.admit(domain(0, 0)).unwrap();
        queue.admit(domain(3, 3)).unwrap();
        queue.admit(domain(2, 4)).unwrap();
        assert_eq!(queue.containment_retired_candidates, 1);
        let ledger = queue.delegation.as_mut().unwrap();
        ledger.native_started(0).unwrap();
        ledger
            .publish_native(
                0,
                NativeOutcome::Completed {
                    unresolved_frontiers: 0,
                },
            )
            .unwrap();
        ledger.publish_delegated(1).unwrap();
        ledger.native_started(2).unwrap();
        queue.next = 2;
        let mut state = State::new(queue, 0, None);
        state.completed = 1;
        state.native_records = 1;
        state.events = 7;
        state.initial_domain_count = 3;
        state.initial_entry_domains_inspected = 1;
        state.records.push(json!({"id":0,"completed":true}));
        state.details.push(json!({"accepted_frontier":3}));
        let mut bytes = Vec::new();
        write(&mut bytes, &state, &[json!({"domain":0})], &[]).unwrap();
        let mut restored: Restored<1> = read(bytes.as_slice()).unwrap();
        assert_eq!(restored.state.queue.next, 2);
        assert_eq!(restored.state.completed, 1);
        assert_eq!(restored.state.events, 7);
        assert_eq!(restored.state.details, state.details);
        assert_eq!(restored.state.records, state.records);
        assert_eq!(restored.inputs, vec![json!({"domain":0})]);
        // Started was not completed/cancelled; its original responsibility is
        // Reserved again and can be dispatched exactly once on restart.
        restored
            .state
            .queue
            .delegation
            .as_mut()
            .unwrap()
            .native_started(2)
            .unwrap();
        assert!(
            restored
                .state
                .queue
                .delegation
                .as_mut()
                .unwrap()
                .native_started(0)
                .is_err()
        );
        for candidate in [domain(3, 3), domain(3, 4), domain(1, 5), domain(0, 6)] {
            assert_eq!(
                state.queue.admit(candidate.clone()),
                restored.state.queue.admit(candidate)
            );
            assert_eq!(
                state.queue.containment_checks,
                restored.state.queue.containment_checks
            );
            assert_eq!(
                state.queue.containment_retired_candidates,
                restored.state.queue.containment_retired_candidates
            );
        }
    }
}
