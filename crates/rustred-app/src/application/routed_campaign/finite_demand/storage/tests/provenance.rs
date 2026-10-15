//! Synthetic wire fixtures test raw-only capability/IO, not query completion.
use super::super::raw::RawDelta;
use super::*;

fn fixture(scratch: &Scratch) -> (StoreWriter, ValidatedGeneration) {
    let mut writer = StoreWriter::create(&scratch.0.join("source"), binding(), limits()).unwrap();
    let generation = writer
        .commit_generated_with_raw(
            vec![key(0), key(1)],
            RawDelta::slices(&[vec![1, 1]], &[vec![0, 1]]),
            state(2, 1),
            &AtomicBool::new(false),
        )
        .unwrap()
        .generation;
    (writer, generation)
}
fn keys(provenance: &RawProvenance) -> Result<Vec<(RawKeyKind, Vec<i64>)>> {
    let mut result = Vec::new();
    provenance.visit_raw_census(
        limits().raw,
        &AtomicBool::new(false),
        |_, _| Ok(()),
        |kind, key| {
            result.push((kind, key.to_vec()));
            Ok(())
        },
    )?;
    Ok(result)
}
#[test]
fn raw_capsule_preserves_captured_bytes_across_source_advance_and_omits_work() {
    let scratch = Scratch::new();
    let (mut writer, first) = fixture(&scratch);
    let captured = RawProvenance::capture(&first);
    writer
        .commit_generated_with_raw(
            vec![key(2), key(3)],
            RawDelta::slices(&[vec![3, 1]], &[vec![2, 1]]),
            state(4, 2),
            &AtomicBool::new(false),
        )
        .unwrap();
    assert_ne!(
        writer.current().unwrap().unwrap().digest(),
        captured.digest()
    );
    // Missing Work bytes cannot be needed by this raw-only export. They would
    // correctly prevent normal completed-generation reopening instead.
    for entry in fs::read_dir(first.directory.join("pages")).unwrap() {
        fs::remove_file(entry.unwrap().path()).unwrap();
    }
    let destination = scratch.0.join("capsule");
    let copied = captured
        .copy_capsule(&destination, limits(), &AtomicBool::new(false))
        .unwrap();
    assert_eq!((copied.digest(), copied.generation()), (first.digest(), 1));
    assert_eq!(copied.state(), first.state());
    assert_eq!(keys(&copied).unwrap(), keys(&captured).unwrap());
    assert!(!destination.join("pages").exists());
    assert!(!destination.join("CURRENT").exists());
    assert!(destination.join("RAW_CURRENT").is_file());
    assert!(ValidatedGeneration::open_generated(&destination, &binding(), limits()).is_err());
    let cold =
        RawProvenance::open_capsule(&destination, &binding(), first.digest(), limits()).unwrap();
    assert_eq!(
        keys(&cold).unwrap(),
        vec![
            (RawKeyKind::Zero, vec![0, 1]),
            (RawKeyKind::Terminal, vec![1, 1])
        ]
    );
    for dir in ["generations", "states", "raw"] {
        for entry in fs::read_dir(destination.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            assert_eq!(
                fs::read(&path).unwrap(),
                fs::read(first.directory.join(dir).join(path.file_name().unwrap())).unwrap()
            );
        }
    }
}
#[test]
fn raw_capsule_expected_identity_marker_and_lazy_page_failures_are_explicit() {
    let scratch = Scratch::new();
    let (_, first) = fixture(&scratch);
    let captured = RawProvenance::capture(&first);
    let destination = scratch.0.join("capsule");
    let copied = captured
        .copy_capsule(&destination, limits(), &AtomicBool::new(false))
        .unwrap();
    assert!(RawProvenance::open_capsule(&destination, &binding(), [9; 32], limits()).is_err());
    let mut wrong = binding();
    wrong.program_identity[0] ^= 1;
    assert!(RawProvenance::open_capsule(&destination, &wrong, first.digest(), limits()).is_err());
    let marker = fs::read(destination.join("RAW_CURRENT")).unwrap();
    fs::write(
        destination.join("RAW_CURRENT"),
        fs::read(first.directory.join("CURRENT")).unwrap(),
    )
    .unwrap();
    assert!(
        RawProvenance::open_capsule(&destination, &binding(), first.digest(), limits()).is_err()
    );
    fs::remove_file(destination.join("RAW_CURRENT")).unwrap();
    assert!(
        RawProvenance::open_capsule(&destination, &binding(), first.digest(), limits()).is_err()
    );
    fs::write(destination.join("RAW_CURRENT"), marker).unwrap();
    let page = fs::read_dir(destination.join("raw"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let original = fs::read(&page).unwrap();
    let mut damaged = original.clone();
    damaged[0] ^= 1;
    fs::write(&page, damaged).unwrap();
    let cold =
        RawProvenance::open_capsule(&destination, &binding(), first.digest(), limits()).unwrap();
    assert_eq!(keys(&cold).unwrap_err().kind, ErrorKind::Corrupt);
    assert_eq!(keys(&copied).unwrap_err().kind, ErrorKind::Corrupt);
    fs::write(&page, original).unwrap();
    fs::remove_file(page).unwrap();
    assert_eq!(
        RawProvenance::open_capsule(&destination, &binding(), first.digest(), limits())
            .unwrap_err()
            .kind,
        ErrorKind::Io
    );
}
#[test]
fn raw_capsule_cancelled_or_failed_copy_never_publishes_marker_or_overwrites() {
    let scratch = Scratch::new();
    let (_, first) = fixture(&scratch);
    let captured = RawProvenance::capture(&first);
    let before = scratch.0.join("before");
    assert_eq!(
        captured
            .copy_capsule(&before, limits(), &AtomicBool::new(true))
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    assert!(!before.exists());
    let midway = scratch.0.join("midway");
    let cancel = AtomicBool::new(false);
    assert_eq!(
        captured
            .copy_with_before_marker(&midway, limits(), &cancel, || {
                cancel.store(true, Ordering::Release);
                Ok(())
            })
            .unwrap_err()
            .kind,
        ErrorKind::Cancelled
    );
    assert!(!midway.join("RAW_CURRENT").exists());
    assert!(midway.join("raw").is_dir());
    assert!(
        captured
            .copy_capsule(&midway, limits(), &AtomicBool::new(false))
            .is_err()
    );
    let failed = scratch.0.join("failed");
    assert!(
        captured
            .copy_with_before_marker(&failed, limits(), &AtomicBool::new(false), || Err(io(
                "injected pre-marker failure"
            )))
            .is_err()
    );
    assert!(!failed.join("RAW_CURRENT").exists());
    assert!(RawProvenance::open_capsule(&failed, &binding(), first.digest(), limits()).is_err());
    let quota = scratch.0.join("quota");
    assert_eq!(
        captured
            .copy_capsule(
                &quota,
                StorageLimits {
                    max_physical_files: 1,
                    ..limits()
                },
                &AtomicBool::new(false)
            )
            .unwrap_err()
            .kind,
        ErrorKind::Resource
    );
    assert!(!quota.exists());
}
#[test]
fn raw_capsule_copy_authenticates_exact_source_bytes_and_preserves_classifier_errors() {
    let scratch = Scratch::new();
    let (_, first) = fixture(&scratch);
    let captured = RawProvenance::capture(&first);
    let destination = scratch.0.join("capsule");
    let copied = captured
        .copy_capsule(&destination, limits(), &AtomicBool::new(false))
        .unwrap();
    let error = copied
        .visit_raw_census(
            limits().raw,
            &AtomicBool::new(false),
            |kind, _| {
                if kind == RawKeyKind::Zero {
                    Err(corrupt("source zero classification rejected"))
                } else {
                    Ok(())
                }
            },
            |_, _| Ok(()),
        )
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::Corrupt);
    assert!(error.to_string().contains("source zero"));
    let page = fs::read_dir(first.directory.join("raw"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let mut bytes = fs::read(&page).unwrap();
    bytes[0] ^= 1;
    fs::write(page, bytes).unwrap();
    let bad = scratch.0.join("bad-copy");
    assert!(
        captured
            .copy_capsule(&bad, limits(), &AtomicBool::new(false))
            .is_err()
    );
    assert!(!bad.join("RAW_CURRENT").exists());
    // The prior capsule is independent of later source corruption/advancement.
    assert_eq!(keys(&copied).unwrap().len(), 2);
}
