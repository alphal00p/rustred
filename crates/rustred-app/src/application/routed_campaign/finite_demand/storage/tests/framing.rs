use super::*;
use rustred::{family::IntegralKey, solver::CandidateRoutedWork};

#[test]
fn finite_store_native_work_roundtrip_preserves_all_i64_values_and_native_order() {
    let mut native = vec![
        CandidateRoutedWork::<2>::Route(IntegralKey::try_new([i64::MIN, i64::MAX]).unwrap()),
        CandidateRoutedWork::Apply {
            owner_sector: [true, false],
            target: IntegralKey::try_new([1, 0]).unwrap(),
        },
        CandidateRoutedWork::Apply {
            owner_sector: [false, true],
            target: IntegralKey::try_new([i64::MIN, 1]).unwrap(),
        },
        CandidateRoutedWork::Route(IntegralKey::try_new([0, 1]).unwrap()),
    ];
    let mut stored = native
        .iter()
        .map(|work| StoredWork::from_native(work).unwrap())
        .collect::<Vec<_>>();
    native.sort();
    stored.sort();
    assert_eq!(
        stored
            .iter()
            .map(|work| work.to_native::<2>().unwrap())
            .collect::<Vec<_>>(),
        native
    );
    let frame = wire::encode(
        wire::PAGE_MAGIC,
        &wire::Page {
            execution: binding(),
            records: stored.clone(),
        },
        limits().max_page_bytes,
    )
    .unwrap();
    let decoded: wire::Page =
        wire::decode(wire::PAGE_MAGIC, &frame, limits().max_page_bytes).unwrap();
    assert_eq!(decoded.records, stored);
    let wrong = CandidateRoutedWork::<1>::Route(IntegralKey::try_new([1, 1]).unwrap());
    assert!(StoredWork::from_native(&wrong).is_err());
    assert!(StoredWork::Route(vec![]).to_native::<2>().is_err());
    assert!(
        StoredWork::Apply {
            owner: vec![false, true],
            powers: vec![1, 1]
        }
        .to_native::<2>()
        .is_err()
    );
}

#[cfg(feature = "capacity-dispatch")]
#[test]
fn finite_store_refuses_nonzero_native_owner_padding() {
    let work = CandidateRoutedWork::<16>::Apply {
        owner_sector: [true; 16],
        target: IntegralKey::try_new([1; 15]).unwrap(),
    };
    assert!(StoredWork::from_native(&work).is_err());
}

#[test]
fn finite_store_tiny_frame_with_enormous_native_vector_count_is_bounded() {
    #[derive(bincode::Encode)]
    struct ClaimedPage {
        execution: RawExecutionBinding,
        records: usize,
    }
    let bytes = wire::encode(
        wire::PAGE_MAGIC,
        &ClaimedPage {
            execution: binding(),
            records: usize::MAX,
        },
        1024,
    )
    .unwrap();
    assert!(bytes.len() < 256);
    let error = match wire::decode::<wire::Page>(wire::PAGE_MAGIC, &bytes, 1024) {
        Ok(_) => panic!("huge vector length was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind, ErrorKind::Corrupt);
    assert!(error.to_string().contains("LimitExceeded"), "{error}");
}

#[test]
fn finite_store_page_descriptor_and_payload_are_checked_exactly() {
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let generation = commit(&mut writer, vec![key(0), key(2)], 2, 1);
    let reference = &generation.manifest.runs[0][0];
    let bytes = fs::read(page_path(&generation, 0)).unwrap();
    let mut page: wire::Page =
        wire::decode(wire::PAGE_MAGIC, &bytes, limits().max_page_bytes).unwrap();
    let mut wrong = reference.clone();
    wrong.first = key(-1);
    assert_eq!(
        wire::validate_page(&page, &wrong, &binding(), limits())
            .unwrap_err()
            .kind,
        ErrorKind::Corrupt
    );
    page.records.reverse();
    assert!(wire::validate_page(&page, reference, &binding(), limits()).is_err());
    page.records = vec![key(0), key(0)];
    assert!(wire::validate_page(&page, reference, &binding(), limits()).is_err());
    for malformed in [
        bytes[..bytes.len() - 1].to_vec(),
        [bytes.clone(), vec![0]].concat(),
    ] {
        assert!(
            wire::decode::<wire::Page>(wire::PAGE_MAGIC, &malformed, limits().max_page_bytes)
                .is_err()
        );
    }
}

#[test]
fn finite_store_changed_manifest_or_state_cannot_authorize_a_cursor() {
    for target in ["CURRENT", "manifest", "state"] {
        let scratch = Scratch::new();
        let path = scratch.0.join("store");
        let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
        let generation = commit(&mut writer, vec![key(1)], 1, 1);
        drop(writer);
        let file = match target {
            "CURRENT" => path.join(wire::CURRENT),
            "manifest" => path
                .join("generations")
                .join(wire::name(&generation.digest(), "rrgen")),
            _ => path
                .join("states")
                .join(wire::name(&generation.manifest.state.digest, "rrstate")),
        };
        let mut bytes = fs::read(&file).unwrap();
        bytes[0] ^= 1;
        fs::write(file, bytes).unwrap();
        assert!(ValidatedGeneration::open_generated(&path, &binding(), limits()).is_err());
    }
}

#[cfg(unix)]
#[test]
fn finite_store_rejects_page_and_directory_symlinks() {
    use std::os::unix::fs::symlink;
    let scratch = Scratch::new();
    let path = scratch.0.join("store");
    let mut writer = StoreWriter::create(&path, binding(), limits()).unwrap();
    let generation = commit(&mut writer, vec![key(1)], 1, 1);
    drop(writer);
    let page = page_path(&generation, 0);
    let outside = scratch.0.join("owned-test-page");
    fs::rename(&page, &outside).unwrap();
    symlink(&outside, &page).unwrap();
    assert!(ValidatedGeneration::open_generated(&path, &binding(), limits()).is_err());
    let alias = scratch.0.join("alias");
    symlink(&path, &alias).unwrap();
    assert!(StoreWriter::resume_generated(&alias, binding(), limits()).is_err());
}
