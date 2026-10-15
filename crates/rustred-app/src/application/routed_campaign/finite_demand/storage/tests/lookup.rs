use super::*;

#[test]
fn finite_store_indexed_cache_promotes_hits_reuses_slots_and_keeps_clear_capacity_charged() {
    let scratch = Scratch::new();
    let policy = StorageLimits {
        max_page_records: 1,
        max_cached_pages: 3,
        ..limits()
    };
    let mut writer = StoreWriter::create(&scratch.0.join("store"), binding(), policy).unwrap();
    let generation = commit(&mut writer, (0..5).map(key).collect(), 5, 1);
    let reader = generation.reader(policy).unwrap();
    let metadata = reader.statistics().unwrap().retained_cache_metadata_bytes;
    assert!(metadata > 0);
    for n in [0, 1, 2, 0, 3, 0, 2] {
        assert_eq!(reader.lookup(&key(n)).unwrap(), Lookup::Present);
        reader.assert_cache_consistent();
    }
    let stats = reader.statistics().unwrap();
    assert_eq!(
        (stats.page_reads, stats.page_cache_hits, stats.evictions),
        (4, 3, 1)
    );
    assert_eq!(stats.retained_pages, 3);
    assert_eq!(reader.lookup(&key(1)).unwrap(), Lookup::Present); // LRU, not the promoted0.
    assert_eq!(reader.statistics().unwrap().page_reads, 5);
    for _ in 0..4 {
        reader.clear_pages().unwrap();
        reader.assert_cache_consistent();
        let empty = reader.statistics().unwrap();
        assert_eq!(empty.retained_page_payload_bytes, 0);
        assert_eq!(empty.retained_cache_metadata_bytes, metadata);
        for n in (0..40).map(|n| n % 5) {
            assert_eq!(reader.lookup(&key(n)).unwrap(), Lookup::Present);
            reader.assert_cache_consistent();
            let stats = reader.statistics().unwrap();
            assert_eq!(stats.retained_cache_metadata_bytes, metadata);
            assert!(stats.retained_page_payload_bytes + metadata <= policy.max_cached_page_bytes);
        }
    }
}

#[test]
fn finite_store_indexed_cache_tiny_grants_and_extreme_page_ceiling_are_bounded() {
    let scratch = Scratch::new();
    let mut writer = StoreWriter::create(&scratch.0.join("store"), binding(), limits()).unwrap();
    let generation = commit(&mut writer, vec![key(0), key(2)], 2, 1);
    for (pages, bytes) in [(0, usize::MAX), (usize::MAX, 0), (usize::MAX, 1)] {
        let reader = generation
            .reader(StorageLimits {
                max_cached_pages: pages,
                max_cached_page_bytes: bytes,
                ..limits()
            })
            .unwrap();
        for _ in 0..2 {
            assert_eq!(reader.lookup(&key(0)).unwrap(), Lookup::Present);
        }
        reader.assert_cache_consistent();
        let stats = reader.statistics().unwrap();
        assert_eq!(
            (
                stats.page_reads,
                stats.page_cache_hits,
                stats.retained_pages
            ),
            (2, 0, 0)
        );
        assert_eq!(stats.retained_cache_metadata_bytes, 0);
    }
    let reader = generation
        .reader(StorageLimits {
            max_cached_pages: usize::MAX,
            ..limits()
        })
        .unwrap();
    assert_eq!(reader.lookup(&key(0)).unwrap(), Lookup::Present);
    assert_eq!(reader.lookup(&key(0)).unwrap(), Lookup::Present);
    reader.assert_cache_consistent();
    let stats = reader.statistics().unwrap();
    assert_eq!(
        (
            stats.retained_pages,
            stats.page_reads,
            stats.page_cache_hits
        ),
        (1, 1, 1)
    );
    assert!(
        stats.retained_cache_metadata_bytes + stats.retained_page_payload_bytes
            <= limits().max_cached_page_bytes
    );
}

#[test]
fn finite_store_indexed_cache_byte_eviction_is_exact_after_metadata_charge() {
    let scratch = Scratch::new();
    let policy = StorageLimits {
        max_page_records: 1,
        max_cached_pages: 3,
        ..limits()
    };
    let mut writer = StoreWriter::create(&scratch.0.join("store"), binding(), policy).unwrap();
    let generation = commit(&mut writer, (0..4).map(key).collect(), 4, 1);
    let probe = generation.reader(policy).unwrap();
    assert_eq!(probe.lookup(&key(0)).unwrap(), Lookup::Present);
    let stats = probe.statistics().unwrap();
    let reader = generation
        .reader(StorageLimits {
            max_cached_page_bytes: stats.retained_cache_metadata_bytes
                + stats.retained_page_payload_bytes,
            ..policy
        })
        .unwrap();
    for n in [0, 1, 2, 3, 0] {
        assert_eq!(reader.lookup(&key(n)).unwrap(), Lookup::Present);
        reader.assert_cache_consistent();
    }
    let actual = reader.statistics().unwrap();
    assert_eq!(
        actual.retained_cache_metadata_bytes,
        stats.retained_cache_metadata_bytes
    );
    assert_eq!(
        (
            actual.page_reads,
            actual.page_cache_hits,
            actual.evictions,
            actual.retained_pages
        ),
        (5, 0, 4, 1)
    );
}

#[test]
fn finite_store_no_cache_and_eviction_still_use_exact_authoritative_disk() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let generation = commit(&mut writer, vec![key(0), key(2), key(4), key(6)], 4, 1);
    let reader = generation.reader(limits()).unwrap();
    for n in [0, 4, 0] {
        assert_eq!(reader.lookup(&key(n)).unwrap(), Lookup::Present);
    }
    let stats = reader.statistics().unwrap();
    assert_eq!(stats.page_reads, 3);
    assert_eq!(stats.evictions, 2);
    assert_eq!(stats.retained_pages, 1);
    assert_eq!(reader.lookup(&key(1)).unwrap(), Lookup::Absent); // Inside a real range.
    reader.clear_pages().unwrap();
    assert_eq!(reader.lookup(&key(0)).unwrap(), Lookup::Present);
    assert_eq!(reader.statistics().unwrap().page_reads, 4);
    let no_cache = generation
        .reader(StorageLimits {
            max_cached_pages: 0,
            max_cached_page_bytes: 0,
            ..limits()
        })
        .unwrap();
    for _ in 0..2 {
        assert_eq!(no_cache.lookup(&key(0)).unwrap(), Lookup::Present);
    }
    let stats = no_cache.statistics().unwrap();
    assert_eq!(stats.page_reads, 2);
    assert_eq!(stats.page_cache_hits, 0);
    assert_eq!(stats.retained_pages, 0);
}

#[test]
fn finite_store_exhausts_overlapping_runs_before_absence_and_refuses_read_grants() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    commit(&mut writer, vec![key(0), key(2)], 2, 1);
    let generation = commit(&mut writer, vec![key(1)], 3, 2);
    let reader = generation.reader(limits()).unwrap();
    assert_eq!(reader.lookup(&key(1)).unwrap(), Lookup::Present);
    assert_eq!(reader.statistics().unwrap().page_reads, 2);
    let limited = generation
        .reader(StorageLimits {
            max_lookup_pages: 1,
            ..limits()
        })
        .unwrap();
    assert_eq!(
        limited.lookup(&key(1)).unwrap_err().kind,
        ErrorKind::Resource
    );
    assert_eq!(limited.statistics().unwrap().absent, 0);
    let bytes = generation
        .reader(StorageLimits {
            max_lookup_bytes: 0,
            ..limits()
        })
        .unwrap();
    assert_eq!(bytes.lookup(&key(1)).unwrap_err().kind, ErrorKind::Resource);
    assert_eq!(bytes.lookup(&key(99)).unwrap(), Lookup::Absent); // Authoritative range negative; no read.
}

#[test]
fn finite_store_missing_or_corrupt_page_is_never_an_eviction_miss() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let generation = commit(&mut writer, vec![key(0), key(2)], 2, 1);
    let reader = generation.reader(limits()).unwrap();
    assert_eq!(reader.lookup(&key(0)).unwrap(), Lookup::Present);
    let page = page_path(&generation, 0);
    let original = fs::read(&page).unwrap();
    let mut corrupt_bytes = original.clone();
    corrupt_bytes[0] ^= 1;
    fs::write(&page, corrupt_bytes).unwrap();
    reader.clear_pages().unwrap();
    assert_eq!(reader.lookup(&key(0)).unwrap_err().kind, ErrorKind::Corrupt);
    fs::write(&page, original).unwrap();
    fs::remove_file(&page).unwrap();
    assert_eq!(reader.lookup(&key(0)).unwrap_err().kind, ErrorKind::Io);
    assert!(ValidatedGeneration::open_generated(&path, &binding(), limits()).is_err());
    assert_eq!(reader.statistics().unwrap().absent, 0);
}

#[test]
fn finite_store_phases_are_distinct_and_global_zeros_are_a_union() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let route = StoredWork::Route(vec![0, 0]);
    let apply = StoredWork::Apply {
        owner: vec![false, false],
        powers: vec![0, 0],
    };
    let first = state(1, 1);
    let zeros = vec![vec![0, 0]];
    let generation = writer
        .commit_generated_with_raw(
            vec![route.clone()],
            super::super::raw::RawDelta::slices(&[], &zeros),
            first,
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    let reader = generation.reader(limits()).unwrap();
    assert_eq!(reader.lookup(&route).unwrap(), Lookup::Present);
    assert_eq!(reader.lookup(&apply).unwrap(), Lookup::Absent);
    let second = state(2, 2);
    let generation = writer
        .commit_generated_with_raw(
            vec![apply.clone()],
            super::super::raw::RawDelta::slices(&[], &zeros),
            second,
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    assert_eq!(
        generation
            .raw_census_directory()
            .stored_references(RawKeyKind::Zero)
            .unwrap(),
        2
    );
    assert_eq!(
        generation
            .visit_raw_census(
                limits().raw,
                &AtomicBool::new(false),
                |_, _| Ok(()),
                |_, _| Ok(())
            )
            .unwrap()
            .zeros,
        1
    );
    let reader = generation.reader(limits()).unwrap();
    assert_eq!(reader.lookup(&route).unwrap(), Lookup::Present);
    assert_eq!(reader.lookup(&apply).unwrap(), Lookup::Present);
    let conflict = StoredWork::Apply {
        owner: vec![true, false],
        powers: vec![0, 0],
    };
    assert_eq!(
        reader.lookup(&conflict).unwrap_err().kind,
        ErrorKind::Corrupt
    );
    assert!(
        writer
            .commit_generated(vec![apply], state(3, 3), &AtomicBool::new(false))
            .is_err()
    );
    assert_eq!(writer.current().unwrap().unwrap().generation(), 2);
}

#[test]
fn finite_store_captured_generation_does_not_follow_a_later_current_pointer() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let first = commit(&mut writer, vec![key(1)], 1, 1);
    let first_reader = first.reader(limits()).unwrap();
    let second = commit(&mut writer, vec![key(2)], 2, 2);
    assert_ne!(first.digest(), second.digest());
    assert_eq!(first_reader.lookup(&key(2)).unwrap(), Lookup::Absent);
    assert_eq!(
        second.reader(limits()).unwrap().lookup(&key(2)).unwrap(),
        Lookup::Present
    );
    let cursor = first.cursor(0).unwrap();
    assert_eq!(cursor.generation(), first.digest());
    assert_eq!(cursor.input_occurrences(), 1);
    assert_eq!(cursor.completed_windows(), 1);
    assert_eq!(cursor.row().origin, "original:0");
    assert_eq!(cursor.source_policy(), [6; 32]);
    assert_eq!(cursor.encoded_cursor(), b"synthetic-wire-cursor-1");
}
