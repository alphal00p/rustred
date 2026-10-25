use super::*;
use rustred::family::IntegralKey;
use std::{collections::BTreeMap, process::Command};

const CHILD_ROOT: &str = "RUSTRED_RUNNING_CACHE_RELATIVE_PATH_CHILD_ROOT";
const TEST_NAME: &str = "application::saved_application::running_cache::path_tests::relative_delta_writer_stays_at_its_lease_after_cwd_changes";

#[test]
fn relative_delta_writer_stays_at_its_lease_after_cwd_changes() {
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        check_relative_delta_in_child(PathBuf::from(root));
        return;
    }

    // Changing cwd affects every thread in a process. Run this fixture in its
    // own filtered test process so parallel tests retain their working directory.
    let scratch = super::super::tests::Scratch::new();
    let root = std::fs::canonicalize(&scratch.0).unwrap();
    std::fs::create_dir(root.join("other")).unwrap();
    let status = Command::new(std::env::current_exe().unwrap())
        .arg(TEST_NAME)
        .arg("--exact")
        .arg("--test-threads=1")
        .env(CHILD_ROOT, &root)
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success(), "relative delta child failed: {status}");
    assert!(root.join("delta/cache.rrindex").is_file());
    assert!(!root.join("other/delta").exists());
}

fn check_relative_delta_in_child(root: PathBuf) {
    let root = std::fs::canonicalize(root).unwrap();
    let original_delta = root.join("delta");
    let other = root.join("other");
    let context = CoefficientContext::try_new(["d"]).unwrap();
    let binding = ApplicationCacheBinding {
        artifact_digest: [44; 32],
        family_fingerprint: "running-cache-relative-path".into(),
        physical_arity: 1,
        layer: "raw-declared-unit-mass-v1".into(),
    };
    let options = SavedApplicationRunningCacheOptions {
        checkpoint_interval: Duration::from_secs(60),
        ..Default::default()
    };
    let mut expected = Vec::new();

    // The first iteration creates the delta; the second resumes its writer.
    // Both must retain the original absolute location after cwd changes.
    for power in [2, 3] {
        std::env::set_current_dir(&root).unwrap();
        let mut session = SavedApplicationRunningCache::open(
            binding.clone(),
            context.clone(),
            Path::new("delta"),
            Vec::new(),
            options,
        )
        .unwrap();
        let canonical_delta = std::fs::canonicalize(&original_delta).unwrap();
        std::env::set_current_dir(&other).unwrap();

        let record = ApplicationCacheRecord {
            key: ApplicationCacheKey::route(&IntegralKey::try_new([power]).unwrap()),
            terms: BTreeMap::from([(IntegralKey::try_new([1]).unwrap(), context.one())]),
            nonzero_conditions: Vec::new(),
        };
        session
            .with_capture(|capture| {
                capture
                    .store_completed(&record, &context)
                    .map_err(|error| AppError::from_native(error, AppErrorKind::Execution))
            })
            .unwrap();
        let control = session.control();
        let report = control.flush().unwrap();
        assert_eq!(report.accepted_records, 1);
        assert_eq!(report.committed_records, 1);
        assert_eq!(report.pending_records, 0);
        let snapshot = control
            .snapshot(&root.join(format!("delta-{power}.rrindex")))
            .unwrap();
        assert_eq!(snapshot.directory, canonical_delta);
        assert!(snapshot.directory.is_absolute());
        assert!(control.close().unwrap().closed);
        drop(session);

        expected.push(record);
        assert!(canonical_delta.join("cache.rrindex").is_file());
        assert_eq!(
            std::fs::read_dir(canonical_delta.join("chunks"))
                .unwrap()
                .count(),
            expected.len(),
        );
        assert!(!other.join("delta").exists());

        // Parse and reopen the descriptor while cwd still points elsewhere.
        let descriptor =
            SavedApplicationRunningCacheSnapshot::from_document(&snapshot.to_document()).unwrap();
        assert_eq!(descriptor.directory, canonical_delta);
        let mut reader = SavedApplicationCachePack::open_snapshot(
            &descriptor.directory,
            &descriptor.binding,
            &descriptor.delta,
            options.storage,
        )
        .unwrap();
        assert_eq!(reader.statistics().records, expected.len());
        for record in &expected {
            assert_eq!(
                reader.lookup(&record.key, &context).unwrap(),
                Some(record.clone()),
            );
        }
    }
}
