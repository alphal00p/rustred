use super::*;

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
    let mut first = state(1, 1);
    first.census.zeros = vec![vec![0, 0]];
    let generation = writer
        .commit_generated(vec![route.clone()], first, &AtomicBool::new(false))
        .unwrap()
        .generation;
    let reader = generation.reader(limits()).unwrap();
    assert_eq!(reader.lookup(&route).unwrap(), Lookup::Present);
    assert_eq!(reader.lookup(&apply).unwrap(), Lookup::Absent);
    let mut second = state(2, 2);
    second.census.zeros = vec![vec![0, 0]];
    let generation = writer
        .commit_generated(vec![apply.clone()], second, &AtomicBool::new(false))
        .unwrap()
        .generation;
    assert_eq!(generation.state().census.zeros.len(), 1);
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
