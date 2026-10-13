//! Orchestration-only controls. The mock does not claim to be an IBP program;
//! native ClosedUnitMassProgram integration is validated separately.
use super::*;
use crate::{AppError, AppErrorKind};
use rustred::{
    algebra::CoefficientContext,
    family::IntegralKey,
    persistence::application_cache::{
        ApplicationCacheBinding, ApplicationCacheKey, ApplicationCacheRecord,
    },
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "rustred-preparation-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Backend<'a> {
    bindings: Vec<ApplicationCacheBinding>,
    context: CoefficientContext,
    admitted: std::cell::Cell<usize>,
    calls: usize,
    configured: usize,
    refused: Option<i64>,
    fail: Option<i64>,
    fail_late_context: bool,
    cancel: Option<&'a AtomicBool>,
    order: Vec<(usize, i64)>,
}
impl Backend<'_> {
    fn new(namespaces: usize) -> Self {
        Self {
            bindings: (0..namespaces)
                .map(|n| ApplicationCacheBinding {
                    artifact_digest: [n as u8; 32],
                    family_fingerprint: format!("mock-preparation-{n}"),
                    physical_arity: 1,
                    layer: "guarded-root-mock-test-only".into(),
                })
                .collect(),
            context: CoefficientContext::try_new(["preparation_runner_d"]).unwrap(),
            admitted: 0.into(),
            calls: 0,
            configured: 0,
            refused: None,
            fail: None,
            fail_late_context: false,
            cancel: None,
            order: Vec::new(),
        }
    }
    fn workload(&self, powers: &[i64]) -> CachePreparationWorkload {
        CachePreparationWorkload::explicit(
            self.bindings.clone(),
            (0..self.bindings.len()).flat_map(|n| {
                powers
                    .iter()
                    .map(move |&p| (n, IntegralKey::try_new([p]).unwrap()))
            }),
            self.bindings.len() * powers.len(),
        )
        .unwrap()
    }
}
impl CachePreparationBackend for Backend<'_> {
    fn bindings(&self) -> &[ApplicationCacheBinding] {
        &self.bindings
    }
    fn context(&self, namespace: usize) -> Result<&CoefficientContext, AppError> {
        self.bindings
            .get(namespace)
            .ok_or_else(|| AppError::input("unknown mock namespace"))?;
        if self.fail_late_context && namespace == 1 && self.calls >= 2 {
            return Err(AppError::input("mock second-namespace checkpoint failure"));
        }
        Ok(&self.context)
    }
    fn configure(&mut self, _: &CachePreparationResolvedOptions) -> Result<(), AppError> {
        self.configured += 1;
        Ok(())
    }
    fn validate_entry(&self, target: &CachePreparationTarget) -> Result<(), AppError> {
        self.admitted.set(self.admitted.get() + 1);
        if self.refused == Some(target.integral().powers()[0]) {
            return Err(AppError::input("mock active scope refused root"));
        }
        Ok(())
    }
    fn validate_completed_record(
        &self,
        _: usize,
        record: &ApplicationCacheRecord,
    ) -> Result<(), AppError> {
        assert_eq!(record.nonzero_conditions, vec![self.context.integer(2)]);
        Ok(())
    }
    fn reduce_guarded_root(
        &mut self,
        target: &CachePreparationTarget,
    ) -> Result<ApplicationCacheRecord, AppError> {
        let power = target.integral().powers()[0];
        self.calls += 1;
        self.order.push((target.namespace(), power));
        if self.fail == Some(power) {
            return Err(AppError::limit("mock native work refusal"));
        }
        if let Some(cancel) = self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        Ok(ApplicationCacheRecord {
            key: ApplicationCacheKey::guarded_root(target.integral()),
            terms: if power == 0 {
                BTreeMap::new()
            } else {
                BTreeMap::from([(
                    IntegralKey::try_new([1]).unwrap(),
                    self.context.integer(power),
                )])
            },
            nonzero_conditions: vec![self.context.integer(2)],
        })
    }
    fn operational_profile(&self) -> Value {
        json!({"mock":true})
    }
    fn statistics(&self) -> Value {
        json!({"calls":self.calls})
    }
}
fn options() -> CachePreparationResolvedOptions {
    CachePreparationOptions {
        ram_bytes: Some(1_000_000),
        disk_bytes: Some(10_000_000),
        ..Default::default()
    }
    .resolve(1, Default::default())
    .unwrap()
}

#[test]
fn preparation_resume_revalidates_roots_and_balances_namespaces() {
    let scratch = Scratch::new();
    let output = scratch.0.join("pack");
    let mut backend = Backend::new(2);
    let workload = backend.workload(&[0, 1, 2]);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(report.completed_roots, 6);
    assert_eq!(report.newly_stored_roots, 6);
    assert_eq!(
        backend.order,
        [(0, 0), (1, 0), (0, 1), (1, 1), (0, 2), (1, 2)]
    );
    let mut resumed = Backend::new(2);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut resumed,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(report.reused_roots, 6);
    assert_eq!(resumed.calls, 0);
    assert_eq!(resumed.admitted.get(), 6);
    resumed.refused = Some(0);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut resumed,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Failed);
    assert_eq!(report.completed_roots, 0);
    assert_eq!(report.reused_roots, 0);
    assert_eq!(resumed.calls, 0);
}

#[test]
fn later_namespace_failure_keeps_exact_durable_preparation_prefix() {
    let scratch = Scratch::new();
    let output = scratch.0.join("pack");
    let mut backend = Backend::new(2);
    backend.fail_late_context = true;
    let workload = backend.workload(&[1]);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Failed);
    assert_eq!(report.total_roots, 2);
    assert_eq!(report.completed_roots, 1);
    assert_eq!(report.newly_stored_roots, 1);
    assert_eq!(backend.calls, 2);
    let mut resumed = Backend::new(2);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut resumed,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(report.completed_roots, 2);
    assert_eq!(report.reused_roots, 1);
    assert_eq!(report.newly_stored_roots, 1);
    assert_eq!(resumed.calls, 1);
}

#[test]
fn completed_pending_roots_survive_cancellation_and_native_refusal() {
    for refusal in [false, true] {
        let scratch = Scratch::new();
        let output = scratch.0.join("pack");
        let cancel = AtomicBool::new(false);
        let mut backend = Backend::new(1);
        if refusal {
            backend.fail = Some(2);
        } else {
            backend.cancel = Some(&cancel);
        }
        let workload = backend.workload(&[1, 2, 3]);
        let report = prepare_application_cache(
            &workload,
            &options(),
            &output,
            &cancel,
            &mut backend,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(
            report.status,
            if refusal {
                CachePreparationStatus::ResourceRefusal
            } else {
                CachePreparationStatus::Cancelled
            }
        );
        assert_eq!(report.completed_roots, 1);
        assert_eq!(report.newly_stored_roots, 1);
        let mut resumed = Backend::new(1);
        let report = prepare_application_cache(
            &workload,
            &options(),
            &output,
            &AtomicBool::new(false),
            &mut resumed,
            |_| Ok(()),
        )
        .unwrap();
        assert_eq!(report.status, CachePreparationStatus::Complete);
        assert_eq!(report.reused_roots, 1);
        assert_eq!(resumed.calls, 2);
    }
}

#[test]
fn workload_identity_is_fixed_but_runtime_budget_can_change_on_resume() {
    let scratch = Scratch::new();
    let output = scratch.0.join("pack");
    let mut backend = Backend::new(1);
    let workload = backend.workload(&[1, 2]);
    let mut immediate = options();
    immediate.time_limit = std::time::Duration::ZERO;
    let report = prepare_application_cache(
        &workload,
        &immediate,
        &output,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::TimeBudget);
    assert_eq!(backend.calls, 0);
    let different = backend.workload(&[1, 2, 3]);
    let error = prepare_application_cache(
        &different,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap_err();
    assert_eq!(error.kind(), AppErrorKind::Input);
    let report = prepare_application_cache(
        &workload,
        &options(),
        &output,
        &AtomicBool::new(false),
        &mut backend,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(backend.calls, 2);
}

#[test]
fn observer_counts_are_durable_and_failed_checkpoint_never_announces_completion() {
    for inject_failure in [false, true] {
        let scratch = Scratch::new();
        let output = scratch.0.join("pack");
        let mut backend = Backend::new(1);
        let workload = backend.workload(&[1, 2, 3]);
        let mut selected = options();
        selected.storage_limits.chunk.max_records = 1;
        selected.batch_records = 1;
        let mut observed = Vec::new();
        let mut injected = false;
        let context = CoefficientContext::try_new(["preparation_runner_d"]).unwrap();
        let report = prepare_application_cache(
            &workload,
            &selected,
            &output,
            &AtomicBool::new(false),
            &mut backend,
            |report| {
                observed.push(report.status);
                let bundle = crate::SavedApplicationCacheBundle::open_generated(
                    &output,
                    workload.bindings(),
                    selected.storage_limits,
                )?;
                let mut reader = bundle.open_reader(&workload.bindings()[0])?;
                let mut durable = 0;
                for target in workload.targets() {
                    if reader
                        .lookup(
                            &ApplicationCacheKey::guarded_root(target.integral()),
                            &context,
                        )?
                        .is_some()
                    {
                        durable += 1;
                    }
                }
                assert!(
                    report.completed_roots <= durable,
                    "reported roots must have a published child checkpoint"
                );
                if inject_failure && !injected {
                    // Deliberate test-only I/O fault after initial setup: the
                    // final metadata checkpoint must reject a nonregular path.
                    let metadata = output.join("preparation.rrcheckpoint");
                    std::fs::remove_file(&metadata).unwrap();
                    std::fs::create_dir(&metadata).unwrap();
                    injected = true;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(report.completed_roots, 3);
        if inject_failure {
            assert_eq!(report.status, CachePreparationStatus::Failed);
            assert!(report.checkpoint_failure.is_some());
            assert!(!observed.contains(&CachePreparationStatus::Complete));
        } else {
            assert_eq!(report.status, CachePreparationStatus::Complete);
            assert_eq!(observed.last(), Some(&CachePreparationStatus::Complete));
        }
    }
}

#[test]
fn inconsistent_resolved_budgets_fail_before_configuration_or_io() {
    let scratch = Scratch::new();
    let output = scratch.0.join("must-not-be-created");
    let mut backend = Backend::new(1);
    let workload = backend.workload(&[1]);
    let mut ram = options();
    ram.retained_coefficient_bytes += 1;
    let mut disk = options();
    disk.storage_limits.max_disk_bytes += 1;
    let mut zero_batch = options();
    zero_batch.batch_records = 0;
    let mut oversized_batch = options();
    oversized_batch.batch_records = oversized_batch.storage_limits.chunk.max_records + 1;
    for selected in [ram, disk, zero_batch, oversized_batch] {
        let error = prepare_application_cache(
            &workload,
            &selected,
            &output,
            &AtomicBool::new(false),
            &mut backend,
            |_| Ok(()),
        )
        .unwrap_err();
        assert_eq!(error.kind(), AppErrorKind::Input);
        assert_eq!(backend.calls, 0);
        assert_eq!(backend.configured, 0);
        assert!(!output.exists());
    }
}

#[test]
fn smaller_batch_target_resumes_a_larger_existing_chunk() {
    let scratch = Scratch::new();
    let output = scratch.0.join("old-layout");
    let mut first = Backend::new(1);
    let workload = first.workload(&(1..=67).collect::<Vec<_>>());
    let mut old = options();
    old.batch_records = old.storage_limits.chunk.max_records;
    let mut calls = 0;
    let report = prepare_application_cache(
        &workload,
        &old,
        &output,
        &AtomicBool::new(false),
        &mut first,
        |_| {
            calls += 1;
            if calls == 66 {
                Err(AppError::new(AppErrorKind::Cancelled, "stop after65 roots"))
            } else {
                Ok(())
            }
        },
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Cancelled);
    assert_eq!(report.completed_roots, 65);
    let small = options();
    assert_eq!(small.batch_records, 64);
    assert_eq!(small.storage_limits, old.storage_limits);
    let bundle = crate::SavedApplicationCacheBundle::open_generated(
        &output,
        workload.bindings(),
        small.storage_limits,
    )
    .unwrap();
    let mut reader = bundle.open_reader(&workload.bindings()[0]).unwrap();
    assert_eq!(reader.statistics().chunks, 1);
    assert_eq!(reader.statistics().records, 65);
    let last = ApplicationCacheKey::guarded_root(&IntegralKey::try_new([65]).unwrap());
    assert!(reader.lookup(&last, &first.context).unwrap().is_some());
    let mut resumed = Backend::new(1);
    let report = prepare_application_cache(
        &workload,
        &small,
        &output,
        &AtomicBool::new(false),
        &mut resumed,
        |_| Ok(()),
    )
    .unwrap();
    assert_eq!(report.status, CachePreparationStatus::Complete);
    assert_eq!(report.reused_roots, 65);
    assert_eq!(report.newly_stored_roots, 2);
    assert_eq!(report.completed_roots, 67);
    assert_eq!(resumed.calls, 2);
    assert_eq!(resumed.admitted.get(), 67);
    assert_eq!(report.options["batch_records"], 64);
}
