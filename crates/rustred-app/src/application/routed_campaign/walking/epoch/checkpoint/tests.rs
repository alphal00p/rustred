//! Pure geometry and writer tests; these do not run a native inspection.
use super::super::super::descendant_closure::Tracker;
use super::super::super::queue::{Domain, Phase};
use super::super::anchors::{AnchorKind, AnchorRecord, AnchorRef, AnchorScope, Lent};
use super::super::dispatch::Refill;
use super::super::job::{Reader, read_image};
use super::super::ledger6::Transition;
use super::super::state::NODE_RESIDUAL;
use super::*;
use rustred::solver::DomainPowerBounds;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};

const HEADER: usize = 28;

pub(super) fn state(count: usize) -> EpochState<2> {
    let mut state = EpochState::new(100_000, usize::MAX, usize::MAX);
    for id in 0..count {
        let point = id as u64;
        let domain = Domain {
            phase: Phase::Apply,
            owner: [true, false],
            lower: vec![point, 0],
            upper: vec![Some(point), Some(0)],
            rank: Some(100),
            powers: DomainPowerBounds::default(),
        };
        super::super::admit_initial(&mut state, &domain).unwrap();
    }
    state.p0 = state.watermark();
    state.tracker = Tracker::new(count);
    state
}

fn bytes(boundary: &MergeBoundary<'_, 2>, section: Section) -> Vec<u8> {
    let (bytes, receipt) = boundary.write_section(Vec::new(), section).unwrap();
    assert_eq!(receipt.bytes, bytes.len() as u64);
    assert_eq!(receipt.blake3, *blake3::hash(&bytes).as_bytes());
    assert_eq!(&bytes[..8], b"EPC6PART");
    assert_eq!(&bytes[8..12], &1u32.to_le_bytes());
    assert_eq!(&bytes[12..16], &2u32.to_le_bytes());
    assert_eq!(&bytes[16..20], &(section as u32).to_le_bytes());
    bytes
}

#[test]
fn geometry_sections_preserve_images_words_live_and_stale_closure() {
    let mut state = state(3);
    state.set_live(1, false);
    state.edges.append_run(0, &[1, 2], false).unwrap();
    state.frontier_counts.insert(2, 7);
    // No forced refresh during save: the previous valid closed bits survive.
    state.tracker.finish(0, true, true);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let domains = bytes(&boundary, Section::Domains);
    let mut reader = Reader::new(&domains[HEADER..]);
    for image in &state.store.domains {
        assert_eq!(read_image::<2>(&mut reader).unwrap(), *image);
    }
    reader.finish().unwrap();
    assert_eq!(&bytes(&boundary, Section::Nodes)[HEADER..], &state.nodes);
    assert_eq!(
        &bytes(&boundary, Section::Live)[HEADER..],
        &5u64.to_le_bytes()
    );
    let ledger = bytes(&boundary, Section::Ledger);
    assert_eq!(
        &ledger[HEADER..],
        state
            .ledger
            .words()
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    );
    let edges = bytes(&boundary, Section::Edges);
    assert_eq!(
        &edges[HEADER..],
        state
            .edges
            .log()
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        &bytes(&boundary, Section::ClosureFlags)[HEADER..],
        state.tracker.node_flags().collect::<Vec<_>>()
    );
    assert_eq!(
        &bytes(&boundary, Section::Frontiers)[HEADER..],
        [2u32.to_le_bytes(), 7u32.to_le_bytes()].concat()
    );
}

#[test]
fn reservation_order_attempts_and_actual_b_are_preserved() {
    let mut state = state(3);
    let mut dispatch = Dispatch::new();
    assert!(matches!(dispatch.refill(&mut state, 3), Refill::Jobs(_)));
    for (id, attempts) in [(0, 2), (1, 1)] {
        state.in_flight.remove(&id);
        state
            .ledger
            .apply(
                id,
                Transition::T7Requeue {
                    d_attempts: attempts,
                    d_guard: 1,
                    last_err: 4,
                },
            )
            .unwrap();
        dispatch.requeue(id, attempts);
    }
    let ledger_before = state.ledger.words().to_vec();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 8).unwrap();
    let encoded = bytes(&boundary, Section::Dispatch);
    let mut reader = Reader::new(&encoded[HEADER..]);
    for scalar in [0, 3, 8, 1, 3, 3] {
        assert_eq!(reader.u64().unwrap(), scalar);
    }
    assert_eq!(reader.u64().unwrap(), 1);
    assert_eq!(reader.u32().unwrap(), 1); // requeue precedes deferred
    assert_eq!(reader.u64().unwrap(), 1);
    assert_eq!(reader.u32().unwrap(), 0);
    assert_eq!(reader.u64().unwrap(), 1);
    assert_eq!(reader.u32().unwrap(), 2);
    assert_eq!(reader.u64().unwrap(), (1 << 40) | 3);
    assert_eq!(reader.u64().unwrap(), 0);
    reader.finish().unwrap();
    let ledger = bytes(&boundary, Section::Ledger);
    assert_eq!(
        &ledger[HEADER..],
        ledger_before
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect::<Vec<_>>()
    );
    assert_eq!(state.ledger.words(), ledger_before);
    assert_eq!(dispatch.queued(), (1, 1));
}

#[test]
fn initial_d_band_anchors_match_existing_layout_without_section_buffer() {
    let mut state = state(3);
    for node in [2, 1] {
        // insertion order deliberately differs from node order
        state
            .anchors
            .push(AnchorRecord {
                node,
                kind: AnchorKind::InitialDBand,
                dispatch_version: 0,
                scope: AnchorScope::DBandCut(5),
                anchors: vec![AnchorRef {
                    anchor: 0,
                    stamp: None,
                    lent: Lent::Full,
                }],
            })
            .unwrap();
    }
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    assert_eq!(
        &bytes(&boundary, Section::Anchors)[HEADER..],
        state.anchors.encode().unwrap()
    );
}

#[test]
fn boundary_refuses_poison_shape_bad_b_and_inconsistent_g2_inventory() {
    let mut state = state(2);
    let dispatch = Dispatch::new();
    for b in [0, 4097] {
        assert!(MergeBoundary::borrow(&state, &dispatch, b).is_err());
    }
    state.poisoned = true;
    assert!(MergeBoundary::borrow(&state, &dispatch, 16).is_err());
    state.poisoned = false;
    state.live[0] |= 1 << 2;
    assert!(MergeBoundary::borrow(&state, &dispatch, 16).is_err());
    state.live[0] = 3;
    state.counters.g2_records = 1;
    assert!(MergeBoundary::borrow(&state, &dispatch, 16).is_err());
    state.counters.g2_records = 0;
    state.nodes.pop();
    assert!(MergeBoundary::borrow(&state, &dispatch, 16).is_err());
}

#[test]
fn boundary_refuses_lost_duplicate_misclassified_and_future_reservations() {
    for mutation in 0..7 {
        let mut state = state(2);
        let mut dispatch = Dispatch::new();
        assert!(matches!(dispatch.refill(&mut state, 2), Refill::Jobs(_)));
        match mutation {
            0 => {
                state.in_flight.remove(&1);
            }
            1 => {
                state.in_flight.clear();
                dispatch.requeue(0, 0);
                dispatch.requeue(0, 0);
            }
            2 => {
                state.in_flight.remove(&1);
                dispatch.requeue(0, 0);
            }
            3 => {
                state.in_flight.get_mut(&0).unwrap().v0 = 1;
            }
            4 => {
                state.in_flight.get_mut(&0).unwrap().seq = 2 << 40;
            }
            5 => {
                state.in_flight.get_mut(&1).unwrap().seq = (1 << 40) | 1;
            }
            6 => {
                state.in_flight.remove(&0);
                dispatch.requeue(0, 2);
            }
            _ => unreachable!(),
        }
        assert!(
            MergeBoundary::borrow(&state, &dispatch, 16).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn boundary_accepts_older_unpublished_job_versions_for_fresh_reissue() {
    let mut state = state(2);
    let mut dispatch = Dispatch::new();
    assert!(matches!(dispatch.refill(&mut state, 2), Refill::Jobs(_)));
    state.k = 1;
    MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
}

#[test]
fn boundary_refuses_residual_bits_even_without_g2_records_or_anchors() {
    for ledger_bit in [false, true] {
        let mut state = state(1);
        if ledger_bit {
            state.ledger.apply(0, Transition::T2Reserve).unwrap();
            state
                .ledger
                .apply(
                    0,
                    Transition::T4Native {
                        epoch: 0,
                        residual: true,
                        dband: false,
                    },
                )
                .unwrap();
        } else {
            state.nodes[0] |= NODE_RESIDUAL;
        }
        let dispatch = Dispatch::new();
        assert_eq!(state.counters.g2_records, 0);
        assert_eq!(state.anchors.len(), 0);
        assert!(MergeBoundary::borrow(&state, &dispatch, 16).is_err());
    }
}

#[derive(Default)]
struct Probe {
    accepted: Vec<u8>,
    max_write: usize,
    fail_after: Option<usize>,
    fail_flush: bool,
    interrupt_once: bool,
}

impl Write for Probe {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        self.max_write = self.max_write.max(input.len());
        if std::mem::take(&mut self.interrupt_once) {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let remaining = self
            .fail_after
            .map_or(usize::MAX, |at| at.saturating_sub(self.accepted.len()));
        if remaining == 0 {
            return Err(io::Error::other("injected write failure"));
        }
        // Exercise write_all's partial-write path, including in the last block.
        let n = input.len().min(97).min(remaining);
        self.accepted.extend_from_slice(&input[..n]);
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush {
            Err(io::Error::other("injected flush failure"))
        } else {
            Ok(())
        }
    }
}

#[test]
fn stream_is_chunk_bounded_and_hashes_exact_partial_writes() {
    let payload = vec![0x5a; BUFFER_BYTES * 3 + 17];
    let mut stream = Stream::new(Probe {
        interrupt_once: true,
        ..Default::default()
    });
    stream.write_all(&payload).unwrap();
    let (probe, digest) = stream.finish().unwrap();
    assert!(probe.max_write <= BUFFER_BYTES);
    assert_eq!(probe.accepted, payload);
    assert_eq!(digest.bytes, payload.len() as u64);
    assert_eq!(digest.blake3, *blake3::hash(&payload).as_bytes());
}

#[test]
fn write_and_flush_failures_never_produce_a_receipt_or_retry_the_buffer() {
    let payload = vec![0x5a; BUFFER_BYTES * 2 + 1];
    for at in [0, 7, BUFFER_BYTES - 1, BUFFER_BYTES, BUFFER_BYTES + 1] {
        let mut probe = Probe {
            fail_after: Some(at),
            ..Default::default()
        };
        let mut stream = Stream::new(&mut probe);
        let failed = stream.write_all(&payload).is_err() || stream.flush().is_err();
        assert!(failed);
        assert!(stream.write_all(b"do not retry").is_err());
        assert!(stream.finish().is_err());
        assert_eq!(probe.accepted.len(), at);
    }
    let mut stream = Stream::new(Probe {
        fail_flush: true,
        ..Default::default()
    });
    stream.write_all(b"written but not flushed").unwrap();
    assert!(stream.finish().is_err());
}

pub(super) struct Directory(pub(super) std::path::PathBuf);
impl Directory {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-epoch-section-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn file_sections_are_immutable_and_not_advertised_as_resumable() {
    let directory = Directory::new();
    let state = state(2);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    assert!(
        boundary
            .write_new_section(&directory.0, 0, Section::Domains)
            .is_err()
    );
    let receipt = boundary
        .write_new_section(&directory.0, 1, Section::Domains)
        .unwrap();
    let path = directory.0.join(Section::Domains.filename(1));
    let original = fs::read(&path).unwrap();
    assert_eq!(receipt.generation, 1);
    assert_eq!(receipt.section, Section::Domains);
    assert_eq!(receipt.digest.bytes, original.len() as u64);
    assert_eq!(receipt.digest.blake3, *blake3::hash(&original).as_bytes());
    assert_eq!(
        boundary
            .write_new_section(&directory.0, 1, Section::Domains)
            .unwrap_err()
            .kind(),
        io::ErrorKind::AlreadyExists
    );
    assert_eq!(fs::read(path).unwrap(), original);
    assert_eq!(fs::read_dir(&directory.0).unwrap().count(), 1);
    assert!(!directory.0.join("latest.json").exists());
    assert!(!directory.0.join(super::super::export::MANIFEST).exists());
}
