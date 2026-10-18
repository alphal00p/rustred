//! Grouping is an operational scheduling choice, not new coverage authority.
use super::finite_backend::{compare_union, finish, policy, query, tree};
use super::*;
use crate::application::routed_campaign::finite_demand::{
    backend::{
        Policy, Session,
        diagnostic::{Mode, OutputBudget, drive},
    },
    storage::ValidatedGeneration,
};
use std::{cell::RefCell, fs, sync::atomic::AtomicBool};

fn overlapping_source(f: &Fixture) -> OwnerDomainWalkRequest {
    let mut matching = OwnerDomainMatchRequest::new(
        f.request.selection_json.clone(),
        json!({
            "schema":"rustred.owner-domain-queries.json.v2",
            "queries":[query("first",3),query("empty",0),query("overlap",5)],
            "query_roles":{"required":["first","empty","overlap"],"auxiliary":[]}
        })
        .to_string(),
    );
    matching.owner_base = f.directory.clone();
    OwnerDomainWalkRequest::new(matching)
}

fn append_overlap(source: &mut OwnerDomainWalkRequest) {
    source.amendments.push(OwnerDomainWalkAmendment {
        path: "grouped-amendment.json".into(),
        text: json!({"schema":OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,"sequence":1,
            "parent":source.checkpoint_binding(),"queries":[query("appended-overlap",4)],
            "query_roles":{"required":["appended-overlap"],"auxiliary":[]}})
        .to_string(),
    });
}

fn exact_output(
    f: &Fixture,
    source: &OwnerDomainWalkRequest,
    p: Policy,
    path: &std::path::Path,
    label: &str,
) -> [Vec<u8>; 3] {
    let output = f.directory.join(label);
    fs::create_dir(&output).unwrap();
    let result = drive::<1>(
        source,
        p,
        Mode::Resume,
        path,
        None,
        &output,
        &OutputBudget::new(1_048_576),
        &AtomicBool::new(false),
        &|_| {},
    )
    .unwrap();
    assert_eq!(result["status"], "complete");
    ["terminals.jsonl", "zeros.jsonl", "selections.jsonl"]
        .map(|name| fs::read(output.join(name)).unwrap())
}

fn assert_row_counts(generation: &ValidatedGeneration, expected: &[u64]) {
    assert_eq!(generation.state().cursors.len(), expected.len());
    assert_eq!(
        generation.state().input_occurrences,
        expected.iter().sum::<u64>()
    );
    for (row, &occurrences) in generation.state().cursors.iter().zip(expected) {
        assert_eq!(row.input_occurrences, occurrences);
        // A grouped commit must preserve the exact one-window enumerator cursor.
        // There is one starting point per window in this fixture.
        assert_eq!(row.completed_windows, occurrences);
    }
}

#[test]
fn finite_worker_pool_reuses_once_and_matches_per_group_with_phase_receipts() {
    let mut expected = None;
    for (reuse_worker_pool, parallel_window_preparation) in
        [(false, false), (true, false), (true, true)]
    {
        let f = Fixture::new();
        let source = overlapping_source(&f);
        let mut p = policy(4);
        p.reuse_worker_pool = reuse_worker_pool;
        p.parallel_window_preparation = parallel_window_preparation;
        p.max_windows_per_batch = 3;
        p.max_resident_work_bytes = 256 * 1024 * 1024;
        let path = f.directory.join("pool-comparison");
        let cancel = AtomicBool::new(false);
        let events = RefCell::new(Vec::new());
        let observer = |event| events.borrow_mut().push(event);
        let mut session = Session::<1>::create(&source, p, &path, &cancel, &observer).unwrap();
        while let Some(window) = session.step(&cancel, &observer).unwrap().window {
            let timings = window.timings;
            assert!(timings.total_seconds >= timings.trace_seconds);
            assert!(timings.total_seconds >= timings.commit_seconds);
            assert!(timings.planning_seconds >= 0. && timings.binding_seconds >= 0.);
            assert!(timings.materialization_seconds >= 0. && timings.cleanup_seconds >= 0.);
            assert!(timings.staging_seconds >= 0.);
            assert_eq!(
                window.resident["records"].as_u64(),
                Some(session.generation().unwrap().state().completed_work)
            );
        }
        assert_eq!(
            events
                .borrow()
                .iter()
                .filter(|e| e["event"] == "finite_worker_pool_ready")
                .count(),
            usize::from(reuse_worker_pool)
        );
        let generation = session.generation().unwrap();
        assert_row_counts(&generation, &[3, 0, 5]);
        drop(session);
        let output = exact_output(&f, &source, p, &path, "exact-pool-output");
        if let Some(previous) = &expected {
            assert_eq!(&output, previous);
        } else {
            expected = Some(output);
        }
        // An already-complete process creates no pool just to shut it down.
        let events = RefCell::new(Vec::new());
        let observer = |event| events.borrow_mut().push(event);
        let mut resumed = Session::<1>::resume(&source, p, &path, &cancel, &observer).unwrap();
        assert!(resumed.step(&cancel, &observer).unwrap().window.is_none());
        assert!(
            !events
                .borrow()
                .iter()
                .any(|e| e["event"] == "finite_worker_pool_ready")
        );
    }
}

#[test]
fn finite_grouping_w1_w4_g1_g8_g64_preserves_exact_union_and_duplicate_occurrences() {
    let mut expected = None;
    for workers in [1, 4] {
        assert!(crate::test_gates::workers_or_skip(
            "finite grouped windows",
            workers
        ));
        for group in [1, 8, 64] {
            for resident in [0, 256 * 1024 * 1024] {
                let f = Fixture::new();
                let source = overlapping_source(&f);
                let mut p = policy(workers);
                p.max_windows_per_batch = group;
                p.max_resident_work_bytes = resident;
                let path = f.directory.join("grouped");
                let events = RefCell::new(Vec::new());
                let mut session =
                    Session::<1>::create(&source, p, &path, &AtomicBool::new(false), &|event| {
                        events.borrow_mut().push(event)
                    })
                    .unwrap();
                let mut commits = 0;
                loop {
                    let step = session
                        .step(&AtomicBool::new(false), &|event| {
                            events.borrow_mut().push(event)
                        })
                        .unwrap();
                    commits += usize::from(step.window.is_some());
                    if step.progress.all_rows_exhausted {
                        break;
                    }
                }
                assert!(session.progress().unwrap().required_complete);
                let generation = session.generation().unwrap();
                assert_row_counts(&generation, &[3, 0, 5]);
                assert_eq!(commits, if group == 1 { 8 } else { 1 });
                // Independent ungrouped one-call native trace checks every exact
                // Work key, terminal/zero key, and selection multiplicity.
                compare_union(&f, &generation, p);
                if group > 1 {
                    let events = events.borrow();
                    let planned = events
                        .iter()
                        .find(|e| e["event"] == "finite_backend_group_planned")
                        .unwrap();
                    assert_eq!(planned["group"]["windows_count"], 8);
                    assert_eq!(planned["group"]["inputs"], 8);
                    assert_eq!(planned["row"], 0);
                    assert_eq!(planned["group"]["last_row"], 2);
                }
                drop(session);
                let actual = exact_output(&f, &source, p, &path, "cold-output");
                if let Some(expected) = &expected {
                    assert_eq!(
                        &actual, expected,
                        "workers={workers}, group={group}, resident={resident}"
                    );
                } else {
                    expected = Some(actual);
                }
            }
        }
    }
}

#[test]
fn finite_grouping_cancel_and_resource_refusal_never_commit_any_group_cursor() {
    for failure in [
        "before-native",
        "before-commit",
        "resource",
        "writer-refusal",
    ] {
        let f = Fixture::new();
        let source = overlapping_source(&f);
        let mut p = policy(4);
        p.max_windows_per_batch = 64;
        p.max_resident_work_bytes = 256 * 1024 * 1024;
        if failure == "resource" {
            p.per_batch_work.max_nodes = 1;
        }
        if failure == "writer-refusal" {
            p.storage.max_delta_records = 0;
        }
        let path = f.directory.join(failure);
        let cancel = AtomicBool::new(false);
        let mut session = Session::<1>::create(&source, p, &path, &cancel, &|_| {}).unwrap();
        let before = session.generation().unwrap();
        let current = fs::read(path.join("CURRENT")).unwrap();
        let error = session
            .step(&cancel, &|event| {
                let stop = (failure == "before-native"
                    && event["event"] == "finite_backend_group_planned")
                    || (failure == "before-commit"
                        && event["event"] == "finite_backend_before_commit");
                if stop {
                    cancel.store(true, Ordering::Release);
                }
            })
            .unwrap_err();
        if matches!(failure, "before-native" | "before-commit") {
            assert_eq!(error.kind(), crate::AppErrorKind::Cancelled);
        }
        assert_eq!(session.generation().unwrap().digest(), before.digest());
        assert_eq!(fs::read(path.join("CURRENT")).unwrap(), current);
        assert_eq!(session.progress().unwrap().input_occurrences, 0);
        drop(session);
        cancel.store(false, Ordering::Release);
        p.per_batch_work = policy(4).per_batch_work;
        p.storage = policy(4).storage;
        let mut resumed = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
        finish(&mut resumed);
        assert_row_counts(&resumed.generation().unwrap(), &[3, 0, 5]);
        compare_union(&f, &resumed.generation().unwrap(), p);
    }
}

#[test]
fn finite_grouping_existing_g1_checkpoint_resumes_g8_and_appends_without_reenumeration() {
    let f = Fixture::new();
    let mut source = overlapping_source(&f);
    let cancel = AtomicBool::new(false);
    let path = f.directory.join("resume");
    let mut p = policy(1);
    let mut initial = Session::<1>::create(&source, p, &path, &cancel, &|_| {}).unwrap();
    initial.step(&cancel, &|_| {}).unwrap();
    initial.step(&cancel, &|_| {}).unwrap();
    let generation = initial.generation().unwrap();
    assert_eq!(generation.state().input_occurrences, 2);
    drop(initial);
    p.max_windows_per_batch = 8;
    p.workers = 4;
    p.max_resident_work_bytes = 256 * 1024 * 1024;
    let before = tree(&path);
    let mut resumed = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
    assert_eq!(resumed.generation().unwrap().digest(), generation.digest());
    assert_eq!(tree(&path), before);
    let grouped = resumed.step(&cancel, &|_| {}).unwrap().window.unwrap();
    assert_eq!(grouped.inputs, 6);
    assert_eq!(grouped.windows_count, 6);
    assert_row_counts(&resumed.generation().unwrap(), &[3, 0, 5]);
    drop(resumed);
    append_overlap(&mut source);
    let mut appended = Session::<1>::resume(&source, p, &path, &cancel, &|_| {}).unwrap();
    assert_eq!(appended.generation().unwrap().state().input_occurrences, 8);
    finish(&mut appended);
    assert_row_counts(&appended.generation().unwrap(), &[3, 0, 5, 4]);
    drop(appended);
    let actual = exact_output(&f, &source, p, &path, "resumed-output");
    let control = f.directory.join("ungrouped-control");
    p.max_windows_per_batch = 1;
    let mut fresh = Session::<1>::create(&source, p, &control, &cancel, &|_| {}).unwrap();
    finish(&mut fresh);
    assert_row_counts(&fresh.generation().unwrap(), &[3, 0, 5, 4]);
    drop(fresh);
    assert_eq!(
        actual,
        exact_output(&f, &source, p, &control, "control-output")
    );
}

#[test]
fn finite_grouping_publication_cold_package_matches_ungrouped_exact_reduction() {
    let f = Fixture::new();
    let cancel = AtomicBool::new(false);
    let mut exact = None;
    for group in [1, 64] {
        let mut p = super::finite_lifecycle::policy();
        p.workers = 4;
        p.max_windows_per_batch = group;
        let mut request = crate::FiniteDemandRunRequest {
            source: overlapping_source(&f),
            policy: p,
            directory: f.directory.join(format!("public-{group}")),
            mode: crate::FiniteDemandMode::Create,
        };
        let solved =
            crate::run_finite_demand_with_progress(request.clone(), &cancel, |_| {}).unwrap();
        assert!(solved.complete);
        assert_eq!(solved.document["committed"]["input_occurrences"], 8);
        assert_eq!(solved.document["committed"]["committed_windows"], 8);
        request.mode = crate::FiniteDemandMode::Resume;
        let published = f.directory.join(format!("publication-{group}"));
        let mut options =
            crate::MasterReductionOptions::new(request.directory.join("store"), &published);
        options.operation = crate::MasterReductionOperation::Publish;
        crate::publish_finite_demand_with_progress(request, &options, &cancel, |_| {}).unwrap();
        let package = f.directory.join(format!("package-{group}"));
        crate::export_saved_application::<4>(&published, &package, Default::default()).unwrap();
        let mut loaded = crate::load_saved_application::<4>(&package, Default::default()).unwrap();
        let one = loaded.family().coefficient_context().one();
        let result = loaded
            .reduce_linear_combination([
                (
                    rustred::family::IntegralKey::try_new([5]).unwrap(),
                    one.clone(),
                ),
                (rustred::family::IntegralKey::try_new([3]).unwrap(), one),
            ])
            .unwrap();
        let value = (result.terms().clone(), result.nonzero_conditions().to_vec());
        if let Some(expected) = &exact {
            assert_eq!(&value, expected);
        } else {
            exact = Some(value);
        }
    }
}
