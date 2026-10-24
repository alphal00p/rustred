//! Keep CP6's typed projection in memory; do not serialize it to JSON text
//! only to parse it again. CP5 still consumes its original JSON-line format.
use super::super::epoch::records::typed::Record;
use super::{RecordRow, result_binding};
use rayon::prelude::*;
use serde_json::Value;
use std::io::{self, BufRead, BufReader, Read};
use std::sync::atomic::{AtomicBool, Ordering};
use symbolica::utils::resources::ResourceBudget;

pub(super) enum RecordValue {
    Typed(Record),
    #[cfg(test)]
    Native(Value),
    Text(String),
}

impl RecordValue {
    pub(super) fn decode(
        self,
        digest: bool,
    ) -> Result<(RecordRow, Option<[u8; 16]>), serde_json::Error> {
        let this = match self {
            Self::Typed(record) => {
                let value = record
                    .project()
                    .map_err(<serde_json::Error as serde::de::Error>::custom)?;
                return decode_value(value, digest);
            }
            other => other,
        };
        if !digest {
            return match this {
                Self::Typed(_) => unreachable!(),
                #[cfg(test)]
                Self::Native(value) => serde_json::from_value(value),
                Self::Text(text) => serde_json::from_str(&text),
            }
            .map(|row| (row, None));
        }
        let value = match this {
            Self::Typed(_) => unreachable!(),
            #[cfg(test)]
            Self::Native(value) => value,
            Self::Text(text) => serde_json::from_str(&text)?,
        };
        decode_value(value, true)
    }
}

fn decode_value(mut value: Value, digest: bool) -> Result<Decoded, serde_json::Error> {
    let digest = digest.then(|| result_binding::record_digest(&mut value));
    Ok((serde_json::from_value(value)?, digest))
}

type Decoded = (RecordRow, Option<[u8; 16]>);

#[derive(Debug)]
pub(super) enum Error {
    Input(io::Error),
    Decode(serde_json::Error),
    Cancelled,
}

/// Only typed CP6 projection/decode is parallel. The coordinator consumes the
/// original authenticated stream and applies every result in input order.
/// One pool lives for the whole load, with a coordinator slot reserved inside
/// the requested worker budget. No CAS state enters these tasks.
pub(super) struct Decoder {
    pool: Option<rayon::ThreadPool>,
    pending: std::vec::IntoIter<Result<Decoded, Error>>,
    #[cfg(test)]
    batch_records: usize,
}

// The private 628,236-record release probe favored 4,096 over 256 at 16/32
// workers (about 5%/14%); retain the independent byte cap for large records.
const BATCH_RECORDS: usize = 4096;
const BATCH_BYTES: u64 = 16 << 20;

impl Decoder {
    pub(super) fn new(workers: usize, native: bool) -> Result<Self, String> {
        let helpers = workers.saturating_sub(1);
        let pool = if native && helpers > 0 {
            Some(
                rayon::ThreadPoolBuilder::new()
                    .num_threads(helpers)
                    .thread_name(|id| format!("record-decode-{id}"))
                    .build()
                    .map_err(|e| format!("record decode worker pool: {e}"))?,
            )
        } else {
            None
        };
        Ok(Self {
            pool,
            pending: Vec::new().into_iter(),
            #[cfg(test)]
            batch_records: BATCH_RECORDS,
        })
    }

    pub(super) fn next<R: Read>(
        &mut self,
        input: &mut Records<R>,
        digest: bool,
        cancellation: &AtomicBool,
    ) -> Option<Result<Decoded, Error>> {
        if let Some(value) = self.pending.next() {
            return Some(value);
        }
        if cancellation.load(Ordering::Relaxed) {
            return Some(Err(Error::Cancelled));
        }
        let Some(pool) = &self.pool else {
            return match input.next() {
                Ok(Some(value)) => Some(value.decode(digest).map_err(Error::Decode)),
                Ok(None) => None,
                Err(error) => Some(Err(Error::Input(error))),
            };
        };
        let first_byte = input.consumed;
        #[cfg(not(test))]
        let record_limit = BATCH_RECORDS;
        #[cfg(test)]
        let record_limit = self.batch_records;
        let mut batch = Vec::with_capacity(record_limit);
        while batch.len() < record_limit && input.consumed - first_byte < BATCH_BYTES {
            if cancellation.load(Ordering::Relaxed) {
                batch.push(Err(Error::Cancelled));
                break;
            }
            match input.next() {
                Ok(Some(value)) => batch.push(Ok(value)),
                Ok(None) => break,
                Err(error) => {
                    // Keep a read/authentication failure behind all earlier
                    // records, including an earlier projection failure.
                    batch.push(Err(Error::Input(error)));
                    break;
                }
            }
        }
        // Capture each refill, since this decoder/pool can outlive a calling
        // request's lexical resource scope.
        let resources = ResourceBudget::capture_current();
        self.pending = pool
            .install(|| {
                resources.run_captured(|| {
                    batch
                        .into_par_iter()
                        .map(|value| {
                            resources.run_captured(|| {
                                if cancellation.load(Ordering::Relaxed) {
                                    return Err(Error::Cancelled);
                                }
                                value?.decode(digest).map_err(Error::Decode)
                            })
                        })
                        .collect::<Vec<_>>()
                })
            })
            .into_iter();
        self.pending.next()
    }
}

pub(super) struct Records<R> {
    input: BufReader<R>,
    remaining: Option<usize>,
    ended: bool,
    consumed: u64,
}

impl<R: Read> Records<R> {
    pub(super) fn new(input: R, native_count: Option<usize>) -> Self {
        Self {
            input: BufReader::with_capacity(1 << 20, input),
            remaining: native_count,
            ended: false,
            consumed: 0,
        }
    }

    pub(super) fn next(&mut self) -> io::Result<Option<RecordValue>> {
        if self.ended {
            return Ok(None);
        }
        if let Some(remaining) = &mut self.remaining {
            if *remaining == 0 {
                // Read through EOF on the same captured authenticated reader,
                // even for empty segments. Never accept unread trailing bytes.
                if self.input.read(&mut [0])? != 0 {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "epoch record segment trailing bytes",
                    ));
                }
                self.ended = true;
                return Ok(None);
            }
            let value = super::super::epoch::records::wire::read(&mut Counted {
                input: &mut self.input,
                consumed: &mut self.consumed,
            })?;
            *remaining -= 1;
            return Ok(Some(RecordValue::Typed(value)));
        }
        loop {
            let mut line = String::new();
            if self.input.read_line(&mut line)? == 0 {
                self.ended = true;
                return Ok(None);
            }
            if line.ends_with('\n') {
                line.pop();
                if line.ends_with('\r') {
                    line.pop();
                }
            }
            if !line.is_empty() {
                return Ok(Some(RecordValue::Text(line)));
            }
        }
    }
}

/// Count logical frame bytes, not buffer prefetch, to bound a decode batch.
/// Existing per-frame wire bounds still apply; one valid oversized frame may
/// exceed the batch target, but is never combined with another afterward.
struct Counted<'a, R> {
    input: &'a mut R,
    consumed: &'a mut u64,
}
impl<R: Read> Read for Counted<'_, R> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        let count = self.input.read(output)?;
        *self.consumed += count as u64;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::routed_campaign::walking::epoch::records::{typed::*, wire};

    fn record(id: u32) -> Record {
        Record {
            authority: Authority {
                id,
                merge_epoch: 1,
                image: Image {
                    route: false,
                    owner: vec![true, false],
                    lower: vec![0, 0],
                    upper: vec![7, u16::MAX],
                    rank: Some(7),
                    powers: Default::default(),
                },
                body: Body::Native(Native {
                    class: 0,
                    kind: 0,
                    v0: 0,
                    distinct_edges: 0,
                    self_edge: false,
                    break_reason: 0,
                    error_kind: 0,
                    err_class: None,
                    panic: false,
                    emitted: 1,
                    accepted: 1,
                    stats_events: 1,
                    has_error: false,
                    frontiers: 0,
                    job_duplicates: 0,
                    known_reuse: 0,
                    resolver: Default::default(),
                    scope: Scope::Whole,
                }),
            },
            diagnostics: Diagnostics {
                stats_json: br#"{"events":1,"successors":0,"extra":{"a":[1,2,3]}}"#.to_vec(),
                refusals: vec![br#"{"reason":"example"}"#.to_vec()],
                ..Default::default()
            },
        }
    }

    fn variants() -> Vec<Record> {
        let mut values = Vec::new();
        for variant in 0..8 {
            let mut value = record(variant);
            let Body::Native(native) = &mut value.authority.body else {
                unreachable!()
            };
            match variant {
                0 => {
                    value.authority.body = Body::Alias {
                        to: 10,
                        exhausted: true,
                    };
                    value.diagnostics = Diagnostics::default();
                }
                1 => (),
                2 => {
                    value.authority.image.route = true;
                    native.kind = 2;
                }
                3 => {
                    native.kind = 1;
                    native.scope = Scope::Initial {
                        anchor: 0,
                        cut: 3,
                        residual: Default::default(),
                    };
                }
                4 => {
                    native.kind = 3;
                    native.scope = Scope::G2 {
                        kind: 1,
                        dispatch: 1,
                        anchors: vec![Anchor {
                            id: 0,
                            stamp: Some(1),
                            low_slice: false,
                        }],
                        pieces: vec![Piece {
                            d_lo: Some(-3),
                            d_hi: Some(3),
                            lower: vec![0, 0],
                            upper: vec![7, u16::MAX],
                        }],
                    };
                }
                5 => {
                    native.kind = 4;
                    native.class = 4;
                    native.frontiers = 1;
                    value
                        .diagnostics
                        .frontiers
                        .push(br#"{"kind":"rescue_abandoned"}"#.to_vec());
                }
                6 => {
                    native.class = 2;
                    native.err_class = Some(0);
                    native.error_kind = 3;
                    native.has_error = true;
                    value.diagnostics.error = Some("native failure".into());
                }
                7 => {
                    value.authority.id = 0;
                    native.kind = 5;
                    native.scope =
                        Scope::FiniteReplay(super::super::super::finite_replay::Recipe {
                            version: super::super::super::finite_replay::OWNER_DOMAIN_WALK_FINITE_REPLAY_VERSION,
                            limits: super::super::super::finite_replay::OwnerDomainWalkFiniteReplayLimits {
                                max_nodes: 10, max_rule_applications: 10,
                                max_transport_calls: 10, max_transport_operations: 10,
                                max_transport_endpoints: 10, max_coalescing_additions: 10,
                                max_positive_layers: 10, max_seed_points: 10, max_seed_bytes: 1024,
                            },
                        });
                }
                _ => unreachable!(),
            }
            value.validate().unwrap();
            values.push(value);
        }
        values
    }

    #[test]
    fn ordered_parallel_decode_matches_all_typed_projections_and_digests() {
        let values = variants();
        let mut bytes = Vec::new();
        let mut expected = Vec::new();
        for id in 0..2 * BATCH_RECORDS + 17 {
            let record = &values[id % values.len()];
            wire::append(record, &mut bytes).unwrap();
            expected.push(record.project().unwrap());
        }
        for workers in [0, 1, 2, 4] {
            for digest in [false, true] {
                let mut decoder = Decoder::new(workers, true).unwrap();
                if let Some(pool) = &decoder.pool {
                    assert_eq!(pool.current_num_threads(), workers - 1);
                }
                let mut input = Records::new(bytes.as_slice(), Some(expected.len()));
                for expected in &expected {
                    let (row, hash) = decoder
                        .next(&mut input, digest, &AtomicBool::new(false))
                        .unwrap()
                        .unwrap();
                    let (reference, reference_hash) = RecordValue::Native(expected.clone())
                        .decode(digest)
                        .unwrap();
                    assert_eq!(
                        serde_json::to_value(row).unwrap(),
                        serde_json::to_value(reference).unwrap()
                    );
                    assert_eq!(hash, reference_hash);
                }
                assert!(
                    decoder
                        .next(&mut input, digest, &AtomicBool::new(false))
                        .is_none()
                );
                assert_eq!(input.consumed, bytes.len() as u64);
            }
        }
    }

    #[test]
    fn ordered_parallel_decode_keeps_errors_eof_and_cancellation() {
        for workers in [1, 4] {
            let mut bytes = Vec::new();
            wire::append(&record(0), &mut bytes).unwrap();
            let mut malformed = record(1);
            malformed.diagnostics.stats_json = b"not JSON".to_vec();
            wire::append(&malformed, &mut bytes).unwrap();
            bytes.push(b'x');
            let mut input = Records::new(bytes.as_slice(), Some(2));
            let mut decoder = Decoder::new(workers, true).unwrap();
            let cancel = AtomicBool::new(false);
            assert_eq!(
                decoder
                    .next(&mut input, false, &cancel)
                    .unwrap()
                    .unwrap()
                    .0
                    .id,
                0
            );
            assert!(matches!(
                decoder.next(&mut input, false, &cancel),
                Some(Err(Error::Decode(_)))
            ));
            assert!(matches!(
                decoder.next(&mut input, false, &cancel),
                Some(Err(Error::Input(_)))
            ));

            let mut input = Records::new(&b""[..], Some(1));
            let mut decoder = Decoder::new(workers, true).unwrap();
            assert!(matches!(
                decoder.next(&mut input, false, &cancel),
                Some(Err(Error::Input(_)))
            ));
            let mut input = Records::new(&b""[..], Some(0));
            let mut decoder = Decoder::new(workers, true).unwrap();
            cancel.store(true, Ordering::Relaxed);
            assert!(matches!(
                decoder.next(&mut input, false, &cancel),
                Some(Err(Error::Cancelled))
            ));
            assert_eq!(input.consumed, 0);
        }
        assert!(Decoder::new(4, false).unwrap().pool.is_none());
    }

    #[test]
    fn ordered_parallel_decode_byte_budget_counts_frames_not_prefetch() {
        let mut oversized = record(0);
        oversized.diagnostics.stats_json = serde_json::to_vec(&serde_json::json!({"events":1,
            "padding":"x".repeat(BATCH_BYTES as usize)}))
        .unwrap();
        let mut bytes = Vec::new();
        wire::append(&oversized, &mut bytes).unwrap();
        let first_frame = bytes.len();
        wire::append(&record(1), &mut bytes).unwrap();
        let mut input = Records::new(bytes.as_slice(), Some(2));
        let mut decoder = Decoder::new(4, true).unwrap();
        let cancel = AtomicBool::new(false);
        assert_eq!(
            decoder
                .next(&mut input, false, &cancel)
                .unwrap()
                .unwrap()
                .0
                .id,
            0
        );
        assert_eq!(input.consumed, first_frame as u64);
        assert_eq!(decoder.pending.len(), 0);
        assert_eq!(
            decoder
                .next(&mut input, false, &cancel)
                .unwrap()
                .unwrap()
                .0
                .id,
            1
        );
        assert!(decoder.next(&mut input, false, &cancel).is_none());
    }

    #[test]
    fn stack_record_parity_matches_original_expanded_domain_checks() {
        use super::super::{Node, Violations, record_node};
        use crate::application::routed_campaign::walking::queue::{CompactDomain, Domain, Phase};
        fn check<const N: usize>() {
            let domain = Domain {
                phase: Phase::Apply,
                owner: std::array::from_fn(|axis| axis == 0),
                lower: vec![0; N],
                upper: (0..N)
                    .map(|axis| match axis {
                        0 => Some(7),
                        1 => None,
                        _ => Some(0),
                    })
                    .collect(),
                rank: Some(7),
                powers: Default::default(),
            };
            let image = CompactDomain::try_from_domain(&domain).unwrap();
            for (key, value) in [
                ("owner", serde_json::json!("10")),
                ("owner", serde_json::json!("1000")),
                ("owner", serde_json::json!("1001")),
                ("owner", serde_json::json!("10é")),
                ("owner", serde_json::json!("x0")),
                ("owner", serde_json::json!("")),
                ("lower", serde_json::json!([0, 0, 0, 0])),
                ("lower", serde_json::json!([0, 0, 1, 0])),
                ("lower", serde_json::json!([0])),
                ("upper", serde_json::json!([7, null, 0, 0])),
                ("upper", serde_json::json!([7, null, null, 0])),
                ("phase", serde_json::json!("Route")),
                ("rank", serde_json::json!(null)),
            ] {
                let mut value_row = record(0).project().unwrap();
                value_row[key] = value;
                let row: RecordRow = serde_json::from_value(value_row).unwrap();
                let owner = row
                    .owner
                    .chars()
                    .map(|c| match c {
                        '0' => Some(false),
                        '1' => Some(true),
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                let owner = owner.and_then(|owner| {
                    crate::application::routed_campaign::storage::restore_array::<_, N>(
                        &owner, false,
                    )
                });
                let lower = crate::application::routed_campaign::storage::restore_array::<_, N>(
                    &row.lower, 0,
                );
                let upper = crate::application::routed_campaign::storage::restore_array::<_, N>(
                    &row.upper,
                    Some(0),
                );
                let expanded = image.expand();
                let same = row.phase == "Apply"
                    && owner == Some(expanded.owner)
                    && lower.as_ref().map(|v| v.as_slice()) == Some(expanded.lower.as_slice())
                    && upper.as_ref().map(|v| v.as_slice()) == Some(expanded.upper.as_slice())
                    && row.rank == expanded.rank
                    && row.power_bounds.bounds() == expanded.powers;
                let mut violations = Violations::new(10);
                record_node(
                    &row,
                    &[image],
                    &mut [Node::MISSING],
                    &mut Default::default(),
                    &mut Default::default(),
                    &mut violations,
                );
                assert_eq!(
                    !violations.by_class.contains_key("domain_parity"),
                    same,
                    "N={N}, field={key}"
                );
            }
        }
        check::<2>();
        #[cfg(feature = "capacity-dispatch")]
        check::<4>();
    }

    /// Explicit diagnostic, never part of the normal test suite. Point only
    /// at a private immutable segment; this opens it read-only and writes no
    /// checkpoint, artifact or sidecar. Use release mode and external affinity.
    #[test]
    #[ignore = "requires an explicitly selected private CP6 record segment"]
    fn cp6_record_decode_release_probe() {
        let path = std::env::var("RUSTRED_RECORD_PROBE_PATH").expect("private segment path");
        let count = std::env::var("RUSTRED_RECORD_PROBE_COUNT")
            .expect("segment count")
            .parse::<usize>()
            .unwrap();
        let workers =
            std::env::var("RUSTRED_RECORD_PROBE_WORKERS").unwrap_or_else(|_| "1,4,32".into());
        let batches =
            std::env::var("RUSTRED_RECORD_PROBE_BATCHES").unwrap_or_else(|_| "256,4096".into());
        let cancel = AtomicBool::new(false);
        let mut reference = None;
        for workers in workers.split(',').map(|v| v.parse::<usize>().unwrap()) {
            for batch in batches.split(',').map(|v| v.parse::<usize>().unwrap()) {
                let mut input = Records::new(std::fs::File::open(&path).unwrap(), Some(count));
                let mut decoder = Decoder::new(workers, true).unwrap();
                decoder.batch_records = batch.max(1);
                let mut rows = 0;
                let mut id_sum = 0u64;
                let mut kinds = std::collections::BTreeMap::<String, usize>::new();
                let started = std::time::Instant::now();
                while let Some(row) = decoder.next(&mut input, false, &cancel) {
                    let (row, digest) = row.unwrap();
                    assert!(digest.is_none());
                    rows += 1;
                    id_sum = id_sum.wrapping_add(row.id as u64);
                    *kinds.entry(row.record_kind).or_default() += 1;
                }
                let seconds = started.elapsed().as_secs_f64();
                assert_eq!(rows, count);
                let summary = (rows, id_sum, kinds);
                if let Some(reference) = &reference {
                    assert_eq!(&summary, reference);
                } else {
                    reference = Some(summary);
                }
                eprintln!(
                    "{}",
                    serde_json::json!({"event":"record_decode_probe", "path":path,
                    "workers":workers,"batch_records":batch,"batch_bytes":BATCH_BYTES,
                    "records":rows,"bytes":input.consumed,"seconds":seconds,
                    "records_per_second":rows as f64 / seconds})
                );
            }
        }
    }
    #[test]
    fn native_reader_rejects_trailing_or_missing_frames() {
        assert!(Records::new(&b""[..], Some(0)).next().unwrap().is_none());
        assert!(Records::new(&b"x"[..], Some(0)).next().is_err());
        assert!(Records::new(&b""[..], Some(1)).next().is_err());
    }

    #[test]
    fn legacy_reader_keeps_lines_semantics() {
        let mut input = Records::new(&b"\n\r\nhello\r\nworld"[..], None);
        for expected in ["hello", "world"] {
            let Some(RecordValue::Text(actual)) = input.next().unwrap() else {
                panic!("missing text")
            };
            assert_eq!(actual, expected);
        }
        assert!(input.next().unwrap().is_none());
        assert!(input.next().unwrap().is_none());
    }
}
