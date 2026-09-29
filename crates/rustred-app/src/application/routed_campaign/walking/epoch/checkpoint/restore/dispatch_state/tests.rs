use super::super::super::super::dispatch::{Dispatch, Refill};
use super::super::super::super::ledger6::Transition;
use super::super::super::super::state::EpochState;
use super::super::super::tests::{Directory, state};
use super::super::super::{Digest, MergeBoundary};
use super::*;
use std::fs;

fn fixture() -> (EpochState<2>, Dispatch) {
    let mut state = state(4);
    state.k = 1;
    let mut dispatch = Dispatch::new();
    assert!(matches!(dispatch.refill(&mut state, 4), Refill::Jobs(_)));
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
    (state, dispatch)
}

fn binding(state: &EpochState<2>) -> Binding<'_> {
    Binding {
        ledger: &state.ledger,
        nodes: &state.nodes,
        k: state.k,
        p0: state.p0,
        lockstep_b: 16,
    }
}

fn receipt(directory: &Path, bytes: &[u8]) -> SectionReceipt {
    fs::write(directory.join(Section::Dispatch.filename(1)), bytes).unwrap();
    SectionReceipt {
        generation: 1,
        section: Section::Dispatch,
        digest: Digest {
            bytes: bytes.len() as u64,
            blake3: *blake3::hash(bytes).as_bytes(),
        },
    }
}

#[test]
fn actual_writer_preserves_order_attempts_and_unfinished_batch_descriptors() {
    let directory = Directory::new();
    let (state, dispatch) = fixture();
    let ledger_before = state.ledger.words().to_vec();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let file = boundary
        .write_new_section(&directory.0, 1, Section::Dispatch)
        .unwrap();
    let saved = read::<2>(&directory.0, &file, 4, binding(&state)).unwrap();
    assert_eq!((saved.session, saved.counter, saved.cursor), (1, 4, 4));
    assert_eq!(saved.requeue.iter().copied().collect::<Vec<_>>(), [1]);
    assert_eq!(saved.deferred.iter().copied().collect::<Vec<_>>(), [0]);
    assert_eq!(saved.in_flight, state.in_flight);
    assert_eq!(
        state.ledger.words(),
        ledger_before,
        "decode never changes attempts/guard or reserves new jobs"
    );
    assert_eq!(state.ledger.get(0).unwrap().counters().unwrap().attempts, 2);
    assert_eq!(state.ledger.get(1).unwrap().counters().unwrap().attempts, 1);
}

#[test]
fn dispatch_counts_sequence_versions_classes_and_membership_mutations_fail() {
    let directory = Directory::new();
    let (state, dispatch) = fixture();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let (original, _) = boundary
        .write_section(Vec::new(), Section::Dispatch)
        .unwrap();
    assert_eq!(original.len(), 28 + 72 + 4 * 4 + 16 * 2);
    for (offset, replacement) in [
        (20, u64::MAX.to_le_bytes().to_vec()), // external Reserved count
        (28, 0u64.to_le_bytes().to_vec()),     // k mismatch
        (36, 3u64.to_le_bytes().to_vec()),     // P0 mismatch
        (44, 8u64.to_le_bytes().to_vec()),     // B mismatch
        (52, 0u64.to_le_bytes().to_vec()),
        (52, SESSION_LIMIT.to_le_bytes().to_vec()),
        (60, SEQUENCE_COUNTER_LIMIT.to_le_bytes().to_vec()),
        (60, 1u64.to_le_bytes().to_vec()), // saved counter below in-flight sequence
        (68, u64::MAX.to_le_bytes().to_vec()),
        (76, u64::MAX.to_le_bytes().to_vec()), // forged queue allocation
        (84, 2u32.to_le_bytes().to_vec()),     // duplicate queue/in-flight member
        (88, 2u64.to_le_bytes().to_vec()),     // wrong deferred inventory
        (96, 1u32.to_le_bytes().to_vec()),     // attempts class mismatch
        (100, 1u64.to_le_bytes().to_vec()),    // wrong unfinished batch cardinality
        (108, 3u32.to_le_bytes().to_vec()),    // duplicate in-flight ID
        (112, ((2u64 << 40) | 3).to_le_bytes().to_vec()), // wrong session
        (112, (1u64 << 40).to_le_bytes().to_vec()), // zero sequence counter
        (120, 0u64.to_le_bytes().to_vec()),    // old version
        (120, 2u64.to_le_bytes().to_vec()),    // future version
        (132, ((1u64 << 40) | 3).to_le_bytes().to_vec()), // duplicate sequence
    ] {
        let mut bytes = original.clone();
        bytes[offset..offset + replacement.len()].copy_from_slice(&replacement);
        let file = receipt(&directory.0, &bytes);
        assert!(
            read::<2>(&directory.0, &file, 4, binding(&state)).is_err(),
            "offset {offset}"
        );
    }
    for delta in [1usize, 4, 16] {
        let file = receipt(&directory.0, &original[..original.len() - delta]);
        assert!(read::<2>(&directory.0, &file, 4, binding(&state)).is_err());
    }
    let mut file = receipt(&directory.0, &original);
    file.digest.blake3[0] ^= 1;
    assert!(read::<2>(&directory.0, &file, 4, binding(&state)).is_err());
}

#[test]
fn empty_batch_is_preserved_but_cursor_gap_and_over_b_batch_are_rejected() {
    let directory = Directory::new();
    let pending = state(1);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&pending, &dispatch, 16).unwrap();
    let (mut bytes, _) = boundary
        .write_section(Vec::new(), Section::Dispatch)
        .unwrap();
    let file = receipt(&directory.0, &bytes);
    let saved = read::<2>(&directory.0, &file, 0, binding(&pending)).unwrap();
    assert_eq!(saved.cursor, 0);
    bytes[68..76].copy_from_slice(&1u64.to_le_bytes());
    let file = receipt(&directory.0, &bytes);
    assert!(read::<2>(&directory.0, &file, 0, binding(&pending)).is_err());
    let (state, dispatch) = fixture();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    let (mut bytes, _) = boundary
        .write_section(Vec::new(), Section::Dispatch)
        .unwrap();
    bytes[44..52].copy_from_slice(&1u64.to_le_bytes());
    let file = receipt(&directory.0, &bytes);
    let mut bound = binding(&state);
    bound.lockstep_b = 1;
    assert!(read::<2>(&directory.0, &file, 4, bound).is_err());
}
