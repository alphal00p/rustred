use super::*;
use crate::application::routed_campaign::walking::epoch::ledger6::{
    Counters, Entry6, Ledger6, Tag,
};

// Deliberately retain the old scalar feeds as an independent byte-stream oracle.
fn scalar_words(digest: &mut blake3::Hasher, words: &[u32]) {
    for word in words {
        digest.update(&word.to_le_bytes());
    }
}

fn scalar_record(digest: &mut blake3::Hasher, id: u32, tag: u8, count: u32) {
    digest.update(&id.to_le_bytes());
    digest.update(&[tag]);
    digest.update(&count.to_le_bytes());
}

fn digest(hash: &blake3::Hasher) -> String {
    hash.finalize().to_hex().to_string()
}

fn observed(store: &EdgeStore) -> (Vec<u32>, u64, u64, u64, String, String) {
    (
        store.log().to_vec(),
        store.runs(),
        store.edges(),
        store.self_edges(),
        store.edge_digest(),
        store.records_digest(),
    )
}

#[test]
fn bounded_feeds_match_scalar_at_tails_and_prefix_boundaries() {
    let patterns = [0, 1, 0x0102_0304, 0x8000_0000, 0xffff_0000, u32::MAX];
    let words: Vec<_> = (0..1025)
        .map(|i| patterns[i % patterns.len()] ^ (i as u32).rotate_left(13))
        .collect();
    for count in [
        0, 1, 2, 8, 14, 15, 16, 17, 64, 253, 254, 255, 256, 257, 258, 511, 512, 513, 1023, 1024,
        1025,
    ] {
        for prefix in [&b""[..], &b"unaligned hash prefix\xff"[..]] {
            let mut reference = blake3::Hasher::new();
            reference.update(prefix);
            let mut actual = reference.clone();
            scalar_words(&mut reference, &words[..count]);
            feed_words(&mut actual, &words[..count]);
            assert_eq!(actual.finalize(), reference.finalize(), "words={count}");
            // A final digest alone is not enough: keep both states appendable.
            scalar_words(&mut reference, &words[17..29]);
            feed_words(&mut actual, &words[17..29]);
            assert_eq!(actual.finalize(), reference.finalize());
        }
    }
    let mut reference = blake3::Hasher::new();
    let mut actual = blake3::Hasher::new();
    for id in 0..257u32 {
        let tag = (id % 256) as u8;
        let count = id.wrapping_mul(0x0102_0304);
        scalar_record(&mut reference, id, tag, count);
        feed_record(&mut actual, id, tag, count);
        assert_eq!(actual.finalize(), reference.finalize(), "record={id}");
    }
}

#[test]
fn edge_run_prefixes_restore_and_continue_with_identical_state() {
    let runs: Vec<_> = [0, 1, 2, 8, 16, 64, 254, 255, 256, 257, 1024]
        .into_iter()
        .enumerate()
        .map(|(i, count)| (i as u32, (0..count).map(|n| n * 3).collect::<Vec<_>>()))
        .collect();
    let mut uninterrupted = EdgeStore::new();
    let mut reference = blake3::Hasher::new();
    let mut prefixes = vec![observed(&uninterrupted)];
    let mut words = Vec::new();
    let mut edges = 0;
    let mut self_edges = 0;
    for (source, targets) in &runs {
        let start = words.len();
        words.extend([*source, targets.len() as u32]);
        words.extend_from_slice(targets);
        scalar_words(&mut reference, &words[start..]);
        uninterrupted.try_reserve(targets.len() + 2).unwrap();
        uninterrupted.append_run(*source, targets, false).unwrap();
        edges += targets.len() as u64;
        self_edges += u64::from(targets.contains(source));
        assert_eq!(uninterrupted.log(), words);
        assert_eq!(uninterrupted.runs(), prefixes.len() as u64);
        assert_eq!(uninterrupted.edges(), edges);
        assert_eq!(uninterrupted.self_edges(), self_edges);
        assert_eq!(uninterrupted.edge_digest(), digest(&reference));
        prefixes.push(observed(&uninterrupted));
    }
    for (count, prefix) in prefixes.iter().enumerate() {
        let mut restored = EdgeStore::from_owned_log(prefix.0.clone(), 4096).unwrap();
        assert_eq!(&observed(&restored), prefix, "restored prefix={count}");
        for (source, targets) in &runs[count..] {
            restored.try_reserve(targets.len() + 2).unwrap();
            restored.append_run(*source, targets, false).unwrap();
        }
        assert_eq!(observed(&restored), observed(&uninterrupted));
    }
    assert_eq!(
        observed(&EdgeStore::from_owned_log(Vec::new(), 0).unwrap()),
        observed(&EdgeStore::new())
    );
}

fn ledger(entries: &[Entry6]) -> Ledger6 {
    let mut ledger = Ledger6::default();
    for entry in entries {
        ledger.restore_word(entry.encode()).unwrap();
    }
    ledger
}

#[test]
fn record_prefixes_restore_native_tags_skip_aliases_and_continue() {
    let entries = [
        Entry6::Native {
            epoch: 1,
            residual: false,
            dband: false,
        },
        Entry6::Alias { to: 5 },
        Entry6::NativeFrontier { epoch: 2 },
        Entry6::NativeError { epoch: 2, err: 1 },
        Entry6::Alias { to: 5 },
        Entry6::Native {
            epoch: 3,
            residual: true,
            dband: false,
        },
        Entry6::Native {
            epoch: 3,
            residual: false,
            dband: true,
        },
    ];
    let ledger = ledger(&entries);
    // Non-ID order and interleaved aliases must remain exactly as published.
    let runs = [(3, 2), (1, 1), (0, 0), (4, 1), (2, 4), (5, 6), (6, 1)];
    let mut uninterrupted = EdgeStore::new();
    let mut reference = blake3::Hasher::new();
    let mut prefixes = vec![observed(&uninterrupted)];
    for (source, count) in runs {
        let targets = match ledger.get(source).unwrap() {
            Entry6::Alias { to } => vec![to],
            _ => (0..count).collect(),
        };
        uninterrupted.append_run(source, &targets, false).unwrap();
        let tag = ledger.tag(source).unwrap();
        if tag != Tag::Alias {
            uninterrupted.fold_record(source, tag as u8, count);
            scalar_record(&mut reference, source, tag as u8, count);
        }
        assert_eq!(uninterrupted.records_digest(), digest(&reference));
        prefixes.push(observed(&uninterrupted));
    }
    for (count, prefix) in prefixes.iter().enumerate() {
        let mut restored = EdgeStore::from_owned_log(prefix.0.clone(), 7).unwrap();
        restored.restore_records_digest(&ledger, &prefix.5).unwrap();
        assert_eq!(&observed(&restored), prefix);
        for &(source, count) in &runs[count..] {
            let targets = match ledger.get(source).unwrap() {
                Entry6::Alias { to } => vec![to],
                _ => (0..count).collect(),
            };
            restored.append_run(source, &targets, false).unwrap();
            let tag = ledger.tag(source).unwrap();
            if tag != Tag::Alias {
                restored.fold_record(source, tag as u8, count);
            }
        }
        assert_eq!(observed(&restored), observed(&uninterrupted));
    }
}

#[test]
fn invalid_runs_refuse_without_changing_the_existing_prefix() {
    let mut store = EdgeStore::new();
    store.append_run(1, &[1, 2], false).unwrap();
    store.fold_record(1, Tag::Native as u8, 2);
    let before = observed(&store);
    for (targets, sealed) in [(&[3, 2][..], false), (&[2, 2][..], false), (&[][..], true)] {
        assert!(store.append_run(3, targets, sealed).is_err());
        assert_eq!(observed(&store), before);
    }
    for malformed in [
        vec![0],
        vec![0, 2, 1],
        vec![0, u32::MAX],
        vec![8, 0],
        vec![0, 1, 8],
        vec![0, 2, 1, 1],
        vec![0, 2, 2, 1],
    ] {
        // The decoder must also refuse a bad run after a valid hashed prefix.
        for prefix in [Vec::new(), store.log().to_vec()] {
            let mut log = prefix;
            log.extend_from_slice(&malformed);
            assert!(EdgeStore::from_owned_log(log, 8).is_err());
        }
    }
    let mut expected = store.edge_digest.clone();
    scalar_words(&mut expected, &[3, 1, 4]);
    store.append_run(3, &[4], false).unwrap();
    assert_eq!(store.edge_digest(), digest(&expected));
}

#[test]
fn record_restore_refusals_preserve_the_existing_appendable_hasher() {
    let native = Entry6::Native {
        epoch: 1,
        residual: false,
        dband: false,
    };
    let good = ledger(&[native, native]);
    let mut store = EdgeStore::new();
    store.append_run(0, &[1], false).unwrap();
    store.append_run(1, &[], false).unwrap();
    store.fold_record(0, Tag::Native as u8, 1);
    store.fold_record(1, Tag::Native as u8, 0);
    let before = observed(&store);
    assert!(store.restore_records_digest(&good, "wrong digest").is_err());
    assert_eq!(observed(&store), before);
    for bad in [
        Entry6::Pending(Counters::default()),
        Entry6::Reserved(Counters::default()),
        Entry6::Exhausted(Counters::default()),
    ] {
        assert!(
            store
                .restore_records_digest(&ledger(&[native, bad]), &before.5)
                .is_err()
        );
        assert_eq!(observed(&store), before);
    }
    assert!(
        store
            .restore_records_digest(&ledger(&[native]), &before.5)
            .is_err()
    );
    assert_eq!(observed(&store), before);
    let mut reference = store.records_digest.clone();
    scalar_record(&mut reference, 2, Tag::NativeFrontier as u8, 7);
    store.fold_record(2, Tag::NativeFrontier as u8, 7);
    assert_eq!(store.records_digest(), digest(&reference));
}
