//! Producer/accounting tests; the mock supplies no mathematical authority.
use super::super::*;
use super::{Backend, Scratch, options};
use crate::SavedApplicationCacheBundle;
use rustred::{family::IntegralKey, persistence::application_cache::ApplicationCacheKey};
use std::sync::atomic::AtomicBool;

#[test]
fn cancellation_during_capture_keeps_child_but_not_interrupted_parent() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("capture-cancel");
    let cancellation = AtomicBool::new(false);
    let mut backend = Backend::new(1);
    backend.capture_children = vec![2, 3];
    backend.cancel = Some(&cancellation);
    backend.cancel_capture_after = Some(1);
    let workload = backend.workload(&[5]);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &destination,
        &cancellation,
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Cancelled);
    assert_eq!(report.completed_roots, 0);
    assert_eq!(report.newly_stored_roots, 0);
    assert_eq!(report.newly_stored_intermediates, 1);
    assert_eq!(backend.calls, 0);
}

#[test]
fn oversized_optional_intermediate_is_reported_without_refusing_requested_root() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("capture-oversize");
    let mut backend = Backend::new(1);
    backend.capture_children = vec![2];
    backend.capture_extra_guards = 100;
    let workload = backend.workload(&[5]);
    let mut options = options();
    options.storage_limits.chunk.diagnostics_enabled = true;
    options.storage_limits.chunk.max_decoded_coefficient_terms = 20;
    let report = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(report.completed_roots, 1);
    assert_eq!(report.newly_stored_intermediates, 0);
    assert_eq!(report.capture_oversized_records, 1);
}

#[test]
fn old_root_only_preparation_checkpoint_cannot_silently_resume_as_captured() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("old-preparation");
    let mut backend = Backend::new(1);
    let workload = backend.workload(&[5]);
    let options = options();
    let report = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.completed_roots, 1);
    let mut writer = crate::SavedApplicationCacheBundleWriter::resume(
        &destination,
        workload.bindings(),
        options.storage_limits,
    )
    .unwrap();
    // The version is checked before decoding the rest of the old frame.
    writer
        .checkpoint_metadata(&bincode::encode_to_vec(1_u32, bincode::config::standard()).unwrap())
        .unwrap();
    drop(writer);
    let calls_before = backend.calls;
    let error = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), crate::AppErrorKind::Input);
    assert!(error.message().contains("new output directory"));
    assert_eq!(backend.calls, calls_before);
}

#[test]
fn failed_parent_retains_completed_descendants_without_selected_acknowledgement() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("capture-prefix");
    let mut backend = Backend::new(1);
    backend.capture_children = vec![2, 3, 4];
    backend.fail = Some(5);
    let workload = backend.workload(&[5]);
    let mut options = options();
    options.batch_records = 1;
    let partial = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(partial.status, CachePreparationStatus::ResourceRefusal);
    assert_eq!(partial.completed_roots, 0);
    assert_eq!(partial.newly_stored_roots, 0);
    assert_eq!(partial.newly_stored_intermediates, 3);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &destination,
        workload.bindings(),
        options.storage_limits,
    )
    .unwrap();
    let mut reader = bundle.open_reader(&workload.bindings()[0]).unwrap();
    assert_eq!(reader.statistics().records, 3);
    for power in [2, 3, 4] {
        assert!(
            reader
                .lookup(
                    &ApplicationCacheKey::guarded_root(&IntegralKey::try_new([power]).unwrap()),
                    &backend.context
                )
                .unwrap()
                .is_some()
        );
    }
    assert!(
        reader
            .lookup(
                &ApplicationCacheKey::guarded_root(&IntegralKey::try_new([5]).unwrap()),
                &backend.context
            )
            .unwrap()
            .is_none()
    );
    backend.fail = None;
    let resumed = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(resumed.status, CachePreparationStatus::Complete);
    assert_eq!(resumed.completed_roots, 1);
    assert_eq!(resumed.newly_stored_roots, 1);
    assert_eq!(resumed.newly_stored_intermediates, 0);
    reader.refresh().unwrap();
    assert_eq!(reader.statistics().records, 4);
}

#[test]
fn pending_descendant_can_acknowledge_later_selected_root_without_duplicate_record() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("capture-pending-selected");
    let mut backend = Backend::new(1);
    backend.capture_children = vec![3, 3];
    let workload = backend.workload(&[2, 3]);
    let options = options();
    let report = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(report.completed_roots, 2);
    assert_eq!(report.newly_stored_roots, 2);
    assert_eq!(report.newly_stored_intermediates, 0);
    assert_eq!(backend.calls, 1);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &destination,
        workload.bindings(),
        options.storage_limits,
    )
    .unwrap();
    assert_eq!(
        bundle
            .open_reader(&workload.bindings()[0])
            .unwrap()
            .statistics()
            .records,
        2
    );
}

#[test]
fn captured_record_does_not_imply_completion_of_any_other_selected_root() {
    let scratch = Scratch::new();
    let destination = scratch.0.join("capture-shared-child");
    let mut backend = Backend::new(1);
    backend.capture_children = vec![2];
    let workload = backend.workload(&[5, 6]);
    let options = options();
    let report = prepare_application_cache(
        &workload,
        &options,
        &destination,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.completed_roots, 2);
    assert_eq!(report.newly_stored_roots, 2);
    assert_eq!(report.newly_stored_intermediates, 1);
    assert_eq!(backend.calls, 2);
    let bundle = SavedApplicationCacheBundle::open_generated(
        &destination,
        workload.bindings(),
        options.storage_limits,
    )
    .unwrap();
    assert_eq!(
        bundle
            .open_reader(&workload.bindings()[0])
            .unwrap()
            .statistics()
            .records,
        3
    );
}
