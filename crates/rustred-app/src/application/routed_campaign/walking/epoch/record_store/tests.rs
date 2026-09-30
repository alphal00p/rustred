//! Binary sidecar lifecycle/failure controls; these do not inspect integrals.
use super::*;
use crate::application::routed_campaign::walking::{
    checkpoint::test_support::test_directory,
    epoch::records::{
        ResolverCounters,
        typed::{Authority, Body, Diagnostics, Image, Native, Scope},
    },
    queue::{CompactDomain, Domain, Phase},
};
use serde_json::json;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        Self(test_directory())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn image(id: u32) -> CompactDomain<1> {
    CompactDomain::try_from_domain(&Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![u64::from(id)],
        upper: vec![None],
        rank: Some(3),
        powers: Default::default(),
    })
    .unwrap()
}

fn record(id: u32) -> Record {
    Record {
        authority: Authority {
            id,
            image: Image::of(&image(id)),
            merge_epoch: 1,
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
                resolver: ResolverCounters::default(),
                scope: Scope::Whole,
            }),
        },
        diagnostics: Diagnostics {
            seconds: 1.25,
            stats_json: serde_json::to_vec(&json!({"events":1,"escaped":"a\nb"})).unwrap(),
            ..Default::default()
        },
    }
}

fn encoded(rows: &[Record]) -> Vec<u8> {
    let mut bytes = Vec::new();
    for row in rows {
        wire::append(row, &mut bytes).unwrap();
    }
    bytes
}

fn read_all(sidecar: &Sidecar) -> Vec<Record> {
    let mut rows = Vec::new();
    sidecar
        .files()
        .for_each_record(|row| {
            rows.push(row);
            Ok(())
        })
        .unwrap();
    rows
}

#[test]
fn binary_generations_seal_exact_bytes_and_restore_only_referenced_segments() {
    let dir = Directory::new();
    let mut sidecar = Sidecar::new(dir.0.clone(), 2);
    let rows: Vec<_> = (0..5).map(record).collect();
    for row in &rows {
        sidecar.push(row).unwrap();
    }
    assert!(sidecar.seal(3, 4).is_err());
    assert!(sidecar.seal(2, 2).is_err());
    let segment = sidecar.seal(2, 3).unwrap().unwrap();
    let expected = encoded(&rows);
    assert_eq!(
        (segment.generation, segment.first, segment.count),
        (2, 0, 5)
    );
    assert_eq!(segment.file, file_name(2));
    assert_eq!(fs::read(dir.0.join(&segment.file)).unwrap(), expected);
    assert_eq!(segment.bytes, expected.len() as u64);
    assert_eq!(segment.blake3, blake3::hash(&expected).to_hex().to_string());
    assert_eq!(sidecar.sealed_total(), 5);
    sidecar.push(&record(5)).unwrap();
    sidecar.write_batch().unwrap();
    assert_eq!(sidecar.total(), 6);
    drop(sidecar);
    // An orphan from an interrupted next generation is not an input to restore.
    assert!(dir.0.join(file_name(3)).exists());
    let mut restored = Sidecar::restored(dir.0.clone(), vec![segment], 4);
    assert_eq!(read_all(&restored), rows);
    restored.push(&record(6)).unwrap();
    let appended = restored.seal(4, 5).unwrap().unwrap();
    assert_eq!((appended.first, appended.count), (5, 1));
    let mut expected = rows;
    expected.push(record(6));
    assert_eq!(read_all(&restored), expected);
    assert_eq!(restored.seal(5, 6).unwrap(), None);
    assert!(!dir.0.join(file_name(5)).exists());
    assert_eq!(restored.generation(), 6);
}

#[test]
fn whole_binary_frames_batch_and_large_diagnostics_do_not_pin_the_buffer() {
    let dir = Directory::new();
    let mut sidecar = Sidecar::new(dir.0.clone(), 2);
    sidecar.push(&record(0)).unwrap();
    let path = dir.0.join(file_name(2));
    assert_eq!(fs::metadata(&path).unwrap().len(), 0);
    assert_eq!(read_all(&sidecar), vec![record(0)]);
    let mut large = record(1);
    large.diagnostics.stats_json =
        serde_json::to_vec(&json!({"pad":"x".repeat(4 * BATCH_BYTES)})).unwrap();
    sidecar.push(&large).unwrap();
    let mut rows = vec![record(0), large];
    assert_eq!(fs::read(&path).unwrap(), encoded(&rows));
    assert!(sidecar.batch.capacity() <= 2 * BATCH_BYTES);
    assert_eq!(sidecar.batched, 0);
    sidecar.push(&record(2)).unwrap();
    rows.push(record(2));
    assert_eq!(sidecar.total(), rows.len());
    assert_eq!(read_all(&sidecar), rows);
    assert_eq!(sidecar.seal(2, 3).unwrap().unwrap().count, 3);
    assert_eq!(fs::read(&path).unwrap(), encoded(&rows));
}

#[test]
fn invalid_authority_cannot_leave_a_partial_frame_or_invent_a_record() {
    let dir = Directory::new();
    let mut sidecar = Sidecar::new(dir.0.clone(), 2);
    sidecar.push(&record(0)).unwrap();
    let before = sidecar.batch.clone();
    let mut malformed = record(1);
    malformed.authority.image.lower.clear();
    let error = sidecar.push(&malformed).unwrap_err();
    assert!(error.contains("serialization"), "{error}");
    assert_eq!(sidecar.batch, before);
    assert_eq!(sidecar.total(), 1);
    assert_eq!(read_all(&sidecar), vec![record(0)]);
    // The publisher treats serialization failure as fatal. This test checks
    // only the sidecar's prefix, not permission to resume the failed publisher.
}

#[test]
fn creation_failure_is_sticky_even_after_the_filesystem_is_repaired() {
    let dir = Directory::new();
    let blocker = dir.0.join("not-a-directory");
    File::create(&blocker).unwrap();
    let mut sidecar = Sidecar::new(blocker.clone(), 2);
    let error = sidecar.push(&record(0)).unwrap_err();
    assert!(
        error.contains("cannot create record sidecar segment"),
        "{error}"
    );
    assert_eq!(sidecar.total(), 0);
    assert!(sidecar.batch.is_empty());
    assert!(sidecar.open.is_none());
    fs::remove_file(&blocker).unwrap();
    fs::create_dir(&blocker).unwrap();
    assert_eq!(sidecar.push(&record(1)).unwrap_err(), error);
    assert_eq!(sidecar.seal(2, 3).unwrap_err(), error);
    assert!(sidecar.closed().is_empty());
    assert!(!blocker.join(file_name(2)).exists());
}

#[test]
fn failed_write_ignores_partial_disk_tail_and_keeps_complete_buffered_records() {
    let dir = Directory::new();
    let mut sidecar = Sidecar::new(dir.0.clone(), 2);
    sidecar.push(&record(0)).unwrap();
    sidecar.write_batch().unwrap();
    sidecar.push(&record(1)).unwrap();
    let path = dir.0.join(file_name(2));
    // Reproduce the on-disk shape after a partial batch write. The subsequent
    // real EBADF failure is privilege-independent; no production I/O seam or
    // host resource limit is needed. `written` must still name whole batches.
    let prefix = sidecar.batch.len() / 2;
    assert!(prefix > wire::HEADER_BYTES);
    OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(&sidecar.batch[..prefix])
        .unwrap();
    sidecar.open.as_mut().unwrap().writer = HashingWriter::new(File::open(&path).unwrap());
    let error = sidecar.seal(2, 3).unwrap_err();
    assert!(error.contains("record sidecar write failed"), "{error}");
    assert_eq!(sidecar.open.as_ref().unwrap().written, 1);
    assert_eq!(sidecar.batched, 1);
    assert_eq!(sidecar.total(), 2);
    assert_eq!(read_all(&sidecar), vec![record(0), record(1)]);
    assert_eq!(sidecar.push(&record(2)).unwrap_err(), error);
    assert_eq!(sidecar.seal(2, 3).unwrap_err(), error);
    assert!(
        sidecar.closed().is_empty(),
        "failed prefix cannot become a manifest segment"
    );
}

#[test]
fn failed_directory_sync_retains_open_tail_and_refuses_later_seals() {
    let dir = Directory::new();
    let path = dir.0.join("run");
    fs::create_dir(&path).unwrap();
    let mut sidecar = Sidecar::new(path.clone(), 2);
    sidecar.push(&record(0)).unwrap();
    sidecar.seal(2, 3).unwrap().unwrap();
    sidecar.push(&record(1)).unwrap();
    sidecar.push(&record(2)).unwrap();
    let moved = dir.0.join("moved");
    fs::rename(&path, &moved).unwrap();
    let error = sidecar.seal(3, 4).unwrap_err();
    fs::rename(&moved, &path).unwrap();
    assert!(
        error.contains("cannot sync checkpoint directory"),
        "{error}"
    );
    assert_eq!(sidecar.closed().len(), 1);
    assert_eq!(sidecar.generation(), 3);
    assert_eq!(sidecar.total(), 3);
    assert_eq!(read_all(&sidecar), (0..3).map(record).collect::<Vec<_>>());
    assert_eq!(sidecar.push(&record(3)).unwrap_err(), error);
    assert_eq!(sidecar.seal(3, 4).unwrap_err(), error);
}

#[test]
fn sealed_binary_segments_reject_digest_length_count_and_frame_corruption() {
    let dir = Directory::new();
    let mut sidecar = Sidecar::new(dir.0.clone(), 2);
    sidecar.push(&record(0)).unwrap();
    sidecar.push(&record(1)).unwrap();
    let segment = sidecar.seal(2, 3).unwrap().unwrap();
    let path = dir.0.join(&segment.file);
    let original = fs::read(&path).unwrap();
    for mutation in 0..6 {
        let mut part = sidecar.files().parts()[0].clone();
        let mut bytes = original.clone();
        match mutation {
            0 => part.sealed.as_mut().unwrap().1 = "0".repeat(64),
            1 => part.sealed.as_mut().unwrap().0 += 1,
            2 => part.count += 1,
            3 => bytes[0] ^= 1,
            4 => {
                bytes.pop();
            }
            5 => bytes.push(0),
            _ => unreachable!(),
        }
        fs::write(&path, bytes).unwrap();
        assert!(
            read_part(&dir.0, &part, &mut |_| Ok(())).is_err(),
            "mutation {mutation}"
        );
    }
    fs::write(&path, original).unwrap();
    assert_eq!(read_all(&sidecar), vec![record(0), record(1)]);
}

#[test]
fn streamed_and_line_projections_equal_materialized_json_and_propagate_errors() {
    let dir = Directory::new();
    let mut sidecar = Sidecar::new(dir.0.clone(), 2);
    let rows = vec![
        record(0),
        Record::alias(1, &image(1), 2, 1, false),
        record(2),
    ];
    for row in &rows[..2] {
        sidecar.push(row).unwrap();
    }
    sidecar.seal(2, 3).unwrap();
    sidecar.push(&rows[2]).unwrap();
    let streamed = Streamed::new(sidecar, Annotations::default());
    let document = json!({"alpha":{"nested":[1,2.5,null]},"domains":null,"zeta":"z","empty":[]});
    let mut bytes = Vec::new();
    streamed.write_json(&document, &mut bytes).unwrap();
    let mut expected = document.clone();
    expected["domains"] = Value::Array(streamed.collect().unwrap());
    assert_eq!(bytes, serde_json::to_vec_pretty(&expected).unwrap());
    let empty = Streamed::new(Sidecar::new(dir.0.clone(), 9), Annotations::default());
    bytes.clear();
    empty.write_json(&document, &mut bytes).unwrap();
    expected["domains"] = json!([]);
    assert_eq!(bytes, serde_json::to_vec_pretty(&expected).unwrap());

    let raw = encoded(&rows);
    let mut reader = JsonLines::new(raw.as_slice(), rows.len());
    let mut lines = Vec::new();
    let mut small = [0u8; 7];
    loop {
        let n = reader.read(&mut small).unwrap();
        if n == 0 {
            break;
        }
        lines.extend_from_slice(&small[..n]);
    }
    let mut expected = Vec::new();
    for row in &rows {
        serde_json::to_writer(&mut expected, &row.project().unwrap()).unwrap();
        expected.push(b'\n');
    }
    assert_eq!(lines, expected);
    assert!(
        JsonLines::new(raw.as_slice(), rows.len() - 1)
            .read_to_end(&mut Vec::new())
            .is_err()
    );
    assert!(
        JsonLines::new(raw.as_slice(), rows.len() + 1)
            .read_to_end(&mut Vec::new())
            .is_err()
    );
    struct Refuse;
    impl Write for Refuse {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected output failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(
        streamed
            .write_json(&document, Refuse)
            .unwrap_err()
            .contains("injected output failure")
    );
}
