use super::super::super::dispatch::Dispatch;
use super::super::super::ledger6::{Counters, Tag};
use super::super::tests::{Directory, state};
use super::super::{Digest, MergeBoundary};
use super::*;
use std::fs;

fn raw(directory: &Path, section: Section, count: u64, payload: &[u8]) -> SectionReceipt {
    let mut bytes = b"EPC6PART".to_vec();
    for word in [1u32, 2, section as u32] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(&count.to_le_bytes());
    bytes.extend_from_slice(payload);
    fs::write(directory.join(section.filename(1)), &bytes).unwrap();
    SectionReceipt {
        generation: 1,
        section,
        digest: Digest {
            bytes: bytes.len() as u64,
            blake3: *blake3::hash(&bytes).as_bytes(),
        },
    }
}

#[test]
fn fixed_sections_roundtrip_actual_runtime_arrays_without_rebuilding_live_index() {
    let directory = Directory::new();
    let mut state = state(3);
    state.set_live(1, false);
    state.edges.append_run(0, &[1, 2], false).unwrap();
    state.edges.append_run(1, &[2], false).unwrap();
    state.tracker.finish(0, true, true);
    let dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &dispatch, 16).unwrap();
    for section in [
        Section::Domains,
        Section::Nodes,
        Section::Live,
        Section::Ledger,
        Section::Edges,
        Section::ClosureFlags,
    ] {
        let receipt = boundary
            .write_new_section(&directory.0, 1, section)
            .unwrap();
        let decoded =
            FixedSection::<2>::open(&directory.0, &receipt, boundary.section_count(section))
                .unwrap();
        match section {
            Section::Domains => {
                let store = decoded.domains().unwrap();
                assert_eq!(store.domains, state.store.domains);
                for (id, image) in store.domains.iter().enumerate() {
                    assert_eq!(
                        store.exact.get(image.digest().0, image, &store.domains),
                        Some(id as u32)
                    );
                }
                assert!(
                    store
                        .buckets
                        .iter()
                        .all(|bucket| bucket.orthant.is_none() && bucket.index.storage().live == 0),
                    "arena decoder cannot guess live membership or historical orthants"
                );
            }
            Section::Nodes => assert_eq!(decoded.flags(false).unwrap(), state.nodes),
            Section::Live => assert_eq!(decoded.live(state.watermark()).unwrap(), state.live),
            Section::Ledger => {
                let ledger = decoded.ledger().unwrap();
                assert_eq!(ledger.words(), state.ledger.words());
                assert_eq!(ledger.counts(), state.ledger.counts());
            }
            Section::Edges => {
                let edges = decoded.edges(state.watermark()).unwrap();
                assert_eq!(edges.log(), state.edges.log());
                assert_eq!(edges.edge_digest(), state.edges.edge_digest());
                assert_eq!(edges.runs(), 2);
                assert_eq!(edges.edges(), 3);
                assert_eq!(edges.pairs().collect::<Vec<_>>(), [(0, 1), (0, 2), (1, 2)]);
                assert_eq!(
                    edges.pairs().clone().collect::<Vec<_>>(),
                    edges.pairs().collect::<Vec<_>>()
                );
            }
            Section::ClosureFlags => assert_eq!(
                decoded.flags(true).unwrap(),
                state.tracker.node_flags().collect::<Vec<_>>()
            ),
            _ => unreachable!(),
        }
    }
}

#[test]
fn forged_counts_and_headers_refuse_before_count_sized_allocation() {
    let directory = Directory::new();
    for section in [
        Section::Domains,
        Section::Nodes,
        Section::Live,
        Section::Ledger,
        Section::Edges,
        Section::ClosureFlags,
        Section::Frontiers,
    ] {
        for count in [u64::MAX, 1 << 48, 10_000_000] {
            let receipt = raw(&directory.0, section, count, &[0; 8]);
            assert!(
                FixedSection::<2>::open(&directory.0, &receipt, count).is_err(),
                "{section:?}: {count}"
            );
        }
    }
    let receipt = raw(&directory.0, Section::Nodes, 1, &[0]);
    assert!(FixedSection::<2>::open(&directory.0, &receipt, 2).is_err());
    assert!(FixedSection::<3>::open(&directory.0, &receipt, 1).is_err());
    assert!(FixedSection::<33>::open(&directory.0, &receipt, 1).is_err());
    assert!(
        FixedSection::<2>::open(&directory.0, &receipt, 1)
            .unwrap()
            .ledger()
            .is_err()
    );
    for offset in [0usize, 8, 12, 16] {
        let mut bytes = fs::read(directory.0.join(receipt.section.filename(1))).unwrap();
        bytes[offset] ^= 1;
        fs::write(directory.0.join(receipt.section.filename(1)), &bytes).unwrap();
        let mutated = SectionReceipt {
            generation: 1,
            section: receipt.section,
            digest: Digest {
                bytes: bytes.len() as u64,
                blake3: *blake3::hash(&bytes).as_bytes(),
            },
        };
        assert!(FixedSection::<2>::open(&directory.0, &mutated, 1).is_err());
        raw(&directory.0, Section::Nodes, 1, &[0]);
    }
}

#[test]
fn ledger_stream_preserves_all_words_and_refuses_malformed_or_unsupported_values() {
    let directory = Directory::new();
    let counters = Counters {
        attempts: 2,
        guard: 1,
        last_err: 4,
        dispatch_class: 9,
    };
    let entries = [
        Entry6::Pending(counters),
        Entry6::Reserved(counters),
        Entry6::Native {
            epoch: 2,
            residual: false,
            dband: true,
        },
        Entry6::NativeFrontier { epoch: 3 },
        Entry6::NativeError { epoch: 4, err: 1 },
        Entry6::Alias { to: 6 },
        Entry6::Exhausted(counters),
        Entry6::Abandoned { epoch: 5 },
    ];
    let words: Vec<_> = entries.iter().map(|entry| entry.encode()).collect();
    let payload: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let receipt = raw(&directory.0, Section::Ledger, 8, &payload);
    let ledger = FixedSection::<2>::open(&directory.0, &receipt, 8)
        .unwrap()
        .ledger()
        .unwrap();
    assert_eq!(ledger.words(), words);
    for tag in Tag::ALL {
        assert_eq!(ledger.counts().get(tag), 1);
    }
    for word in [
        (7 << 61) | (1 << 48), // Abandoned has only the low48 epoch payload.
        1 << 28,
        Entry6::Alias { to: 0 }.encode(),
        Entry6::Alias { to: 1 }.encode(),
    ] {
        let receipt = raw(&directory.0, Section::Ledger, 1, &word.to_le_bytes());
        assert!(
            FixedSection::<2>::open(&directory.0, &receipt, 1)
                .unwrap()
                .ledger()
                .is_err()
        );
    }
    let residual = Entry6::Native {
        epoch: 1,
        residual: true,
        dband: false,
    }
    .encode();
    let receipt = raw(&directory.0, Section::Ledger, 1, &residual.to_le_bytes());
    assert_eq!(
        FixedSection::<2>::open(&directory.0, &receipt, 1)
            .unwrap()
            .ledger()
            .unwrap()
            .words(),
        &[residual]
    );
    let mut ledger = Ledger6::default();
    ledger.restore_word(entries[0].encode()).unwrap();
    let counts = ledger.counts();
    assert!(ledger.restore_word((7 << 61) | (1 << 48)).is_err());
    assert_eq!(ledger.words(), &[entries[0].encode()]);
    assert_eq!(ledger.counts(), counts);
}

#[test]
fn geometry_flags_live_and_edge_run_mutations_fail_closed() {
    let directory = Directory::new();
    for (section, closure, value) in [
        (Section::Nodes, false, 4),
        (Section::Nodes, false, 32),
        (Section::ClosureFlags, true, 8),
    ] {
        let receipt = raw(&directory.0, section, 1, &[value]);
        assert!(
            FixedSection::<2>::open(&directory.0, &receipt, 1)
                .unwrap()
                .flags(closure)
                .is_err()
        );
    }
    // Bit 16 now marks a G2 residual; unrelated high bits remain rejected.
    let receipt = raw(&directory.0, Section::Nodes, 1, &[16]);
    assert_eq!(
        FixedSection::<2>::open(&directory.0, &receipt, 1)
            .unwrap()
            .flags(false)
            .unwrap(),
        [16]
    );
    let receipt = raw(&directory.0, Section::Live, 1, &8u64.to_le_bytes());
    assert!(
        FixedSection::<2>::open(&directory.0, &receipt, 1)
            .unwrap()
            .live(3)
            .is_err()
    );
    assert!(
        FixedSection::<2>::open(&directory.0, &receipt, 1)
            .unwrap()
            .live(65)
            .is_err()
    );
    for words in [
        vec![0],
        vec![0, u32::MAX, 1],
        vec![3, 0],
        vec![0, 1, 3],
        vec![0, 2, 2, 1],
        vec![0, 2, 1, 1],
    ] {
        let payload: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
        let receipt = raw(&directory.0, Section::Edges, 1, &payload);
        assert!(
            FixedSection::<2>::open(&directory.0, &receipt, 1)
                .and_then(|section| section.edges(3))
                .is_err()
        );
    }
    let state = state(1);
    let boundary_dispatch = Dispatch::new();
    let boundary = MergeBoundary::borrow(&state, &boundary_dispatch, 16).unwrap();
    let (mut bytes, _) = boundary
        .write_section(Vec::new(), Section::Domains)
        .unwrap();
    let image = bytes[28..].to_vec();
    let receipt = raw(
        &directory.0,
        Section::Domains,
        2,
        &[image.as_slice(), image.as_slice()].concat(),
    );
    assert!(
        FixedSection::<2>::open(&directory.0, &receipt, 2)
            .unwrap()
            .domains()
            .is_err()
    );
    bytes[28] = 2; // Noncanonical phase; use the existing job-image decoder.
    let receipt = raw(&directory.0, Section::Domains, 1, &bytes[28..]);
    assert!(
        FixedSection::<2>::open(&directory.0, &receipt, 1)
            .unwrap()
            .domains()
            .is_err()
    );
}

#[test]
fn edge_header_counts_runs_not_words_and_exact_inventory_is_checked() {
    let directory = Directory::new();
    // Two runs (one empty, one two-target), six final runtime words.
    let words = [0u32, 0, 1, 2, 0, 2];
    let payload: Vec<_> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    let receipt = raw(&directory.0, Section::Edges, 2, &payload);
    let decoded = FixedSection::<2>::open(&directory.0, &receipt, 2)
        .unwrap()
        .edges(3)
        .unwrap();
    assert_eq!(decoded.log(), words);
    assert_eq!((decoded.runs(), decoded.edges()), (2, 2));
    // Both claims fit the conservative minimum, but disagree with the runs.
    for count in [1, 3] {
        let receipt = raw(&directory.0, Section::Edges, count, &payload);
        assert!(
            FixedSection::<2>::open(&directory.0, &receipt, count)
                .unwrap()
                .edges(3)
                .is_err()
        );
    }
    for (count, bytes) in [(0, payload.as_slice()), (1, &[][..]), (1, &[0u8; 9][..])] {
        let receipt = raw(&directory.0, Section::Edges, count, bytes);
        assert!(FixedSection::<2>::open(&directory.0, &receipt, count).is_err());
    }
    let receipt = raw(&directory.0, Section::Edges, 0, &[]);
    assert_eq!(
        FixedSection::<2>::open(&directory.0, &receipt, 0)
            .unwrap()
            .edges(0)
            .unwrap()
            .runs(),
        0
    );
}
