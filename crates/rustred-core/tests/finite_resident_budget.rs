//! Public resident-budget regression; no integral solving or CAS is involved.
use rustred::persistence::finite_work::{
    GeneratedDirectoryLimits, GeneratedExecutionBinding, GeneratedPageProvider,
    GeneratedPageReadUsage, GeneratedWorkErrorKind, GeneratedWorkPageRef, GeneratedWorkResult,
    TrustedGeneratedResidentWork, TrustedGeneratedWorkDirectory, ValidatedWorkPage,
};
use std::sync::{Arc, atomic::AtomicBool};

struct NoPages;

impl GeneratedPageProvider for NoPages {
    fn visit_validated_page(
        &self,
        _: &GeneratedWorkPageRef,
        _: u64,
        _: &mut dyn FnMut(&ValidatedWorkPage),
    ) -> GeneratedWorkResult<GeneratedPageReadUsage> {
        panic!("an empty trusted directory must not request a page");
    }
}

#[test]
fn metadata_rebind_checks_commit_peak_and_installs_successor_reservation() {
    let predecessor = [1; 32];
    let successor = [3; 32];
    let execution = GeneratedExecutionBinding {
        physical_arity: 1,
        program_identity: [2; 32],
    };
    let limits = GeneratedDirectoryLimits {
        max_arity: 1,
        max_runs: usize::MAX,
        max_pages: usize::MAX,
        max_directory_payload_bytes: usize::MAX,
        max_page_records: 1,
        max_page_bytes: 1024,
        max_referenced_bytes: u64::MAX,
        max_lookup_pages: usize::MAX,
        max_lookup_bytes: u64::MAX,
    };
    let directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        execution.clone(),
        predecessor,
        Arc::new(Vec::new()),
        limits,
    )
    .unwrap();
    let cancel = AtomicBool::new(false);
    let prepare = || {
        TrustedGeneratedResidentWork::hydrate(&directory, &NoPages, 1200, 400, &cancel)
            .unwrap()
            .unwrap()
            .prepare_metadata_only(predecessor)
            .unwrap()
    };

    // Retaining the successor would fit; holding both directories does not.
    assert!(
        prepare()
            .refresh_coexisting_bytes(1201, 900)
            .unwrap()
            .is_none()
    );
    // Equality at the live-byte ceiling is permitted, then only the successor
    // reservation remains after the persistence adapter adopts its generation.
    let resident = prepare()
        .refresh_coexisting_bytes(1200, 700)
        .unwrap()
        .unwrap()
        .after_durable_commit(successor);
    let statistics = resident.statistics().unwrap();
    assert_eq!(statistics.reserved_coexisting_bytes, 700);
    assert_eq!(statistics.max_live_payload_bytes, 1200);
    assert_eq!(statistics.records, 0);
    assert_eq!(statistics.retained_payload_estimate, 0);
    let successor_directory = TrustedGeneratedWorkDirectory::try_from_trusted_generated(
        execution,
        successor,
        Arc::new(Vec::new()),
        limits,
    )
    .unwrap();
    resident.validate_directory(&successor_directory).unwrap();
    assert!(resident.validate_directory(&directory).is_err());
    assert_eq!(
        prepare()
            .refresh_coexisting_bytes(600, 700)
            .err()
            .unwrap()
            .kind,
        GeneratedWorkErrorKind::Corrupt
    );
}
