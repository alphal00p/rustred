//! Resource policy changes may reopen the same trusted CURRENT. Source/rule
//! identities and exact completed Work remain bound to the original input.
use super::finite_backend::{finish, policy, query, tree};
use super::*;
use crate::application::routed_campaign::finite_demand::{
    backend::{
        Policy, Session,
        diagnostic::{Mode, OutputBudget, drive},
    },
    lifecycle::operation_limits::OperationLimits,
};
use std::{fs, sync::atomic::AtomicBool};

fn source(f: &Fixture) -> OwnerDomainWalkRequest {
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v2",
        "queries":[query("original",5),query("empty",0)],
        "query_roles":{"required":["original","empty"],"auxiliary":[]}});
    let mut matching =
        OwnerDomainMatchRequest::new(f.request.selection_json.clone(), queries.to_string());
    matching.owner_base = f.directory.clone();
    OwnerDomainWalkRequest::new(matching)
}

fn raw_output(
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

#[test]
fn old_operation_policy_resumes_with_unbounded_resources_and_identical_completed_work() {
    let f = Fixture::new();
    let source = source(&f);
    let mut old = policy(1);
    old.operation_limits = None;
    old.max_resident_work_bytes = 256 * 1024 * 1024;
    let mut opened = old;
    opened.operation_limits = Some(OperationLimits::default());
    opened.storage.max_manifest_bytes = usize::MAX;
    opened.storage.max_state_bytes = usize::MAX;
    opened.storage.max_transient_bytes = 600_000_000_000;
    opened.max_resident_work_bytes = 300_000_000_000;
    let cancel = AtomicBool::new(false);
    let path = f.directory.join("policy-upgrade");
    let mut session = Session::<1>::create(&source, old, &path, &cancel, &|_| {}).unwrap();
    session.step(&cancel, &|_| {}).unwrap();
    let prior = session.generation().unwrap();
    assert_eq!(prior.state().input_occurrences, 1);
    drop(session);
    let before = tree(&path);
    let hydrated = AtomicBool::new(false);
    let mut session = Session::<1>::resume(&source, opened, &path, &cancel, &|event| {
        if event["event"] == "finite_backend_resident_hydrated" {
            assert_eq!(event["status"], "active");
            assert!(
                event["resident"]["reserved_coexisting_bytes"]
                    .as_u64()
                    .unwrap()
                    < 1_000_000_000
            );
            hydrated.store(true, Ordering::Relaxed);
        }
    })
    .unwrap();
    assert!(hydrated.load(Ordering::Relaxed));
    assert_eq!(
        tree(&path),
        before,
        "operational change alone must not rewrite CURRENT"
    );
    assert_eq!(session.generation().unwrap().digest(), prior.digest());
    finish(&mut session);
    let final_generation = session.generation().unwrap();
    drop(session);
    let control = f.directory.join("policy-control");
    let mut reference = Session::<1>::create(&source, old, &control, &cancel, &|_| {}).unwrap();
    finish(&mut reference);
    let reference_generation = reference.generation().unwrap();
    assert_eq!(
        final_generation.digest(),
        reference_generation.digest(),
        "operational policy is not a semantic identity"
    );
    assert_eq!(final_generation.state(), reference_generation.state());
    drop(reference);
    assert_eq!(
        raw_output(&f, &source, opened, &path, "upgraded-census"),
        raw_output(&f, &source, old, &control, "old-census")
    );
}

#[test]
fn explicit_operation_refusal_retains_current_then_widening_completes_exactly() {
    let f = Fixture::new();
    let source = source(&f);
    let mut normal = policy(1);
    normal.operation_limits = Some(OperationLimits::default());
    let path = f.directory.join("operation-refusal");
    let cancel = AtomicBool::new(false);
    let mut session = Session::<1>::create(&source, normal, &path, &cancel, &|_| {}).unwrap();
    session.step(&cancel, &|_| {}).unwrap(); // terminal I(1), no specialization
    let prior = session.generation().unwrap();
    drop(session);
    let mut capped = normal;
    let mut operations = OperationLimits::default();
    operations
        .reduction
        .indexed_algebra
        .max_specialization_power_operations = 0;
    capped.operation_limits = Some(operations);
    let before = tree(&path);
    let mut session = Session::<1>::resume(&source, capped, &path, &cancel, &|_| {}).unwrap();
    let error = session.step(&cancel, &|_| {}).unwrap_err();
    assert!(error.to_string().contains("specialization"), "{error}");
    assert_eq!(session.generation().unwrap().digest(), prior.digest());
    assert_eq!(session.progress().unwrap().input_occurrences, 1);
    drop(session);
    assert_eq!(
        tree(&path),
        before,
        "failed operation cannot write a successor generation"
    );
    let mut session = Session::<1>::resume(&source, normal, &path, &cancel, &|_| {}).unwrap();
    finish(&mut session);
    let completed = session.generation().unwrap();
    drop(session);
    let control = f.directory.join("operation-refusal-control");
    let mut reference = Session::<1>::create(&source, normal, &control, &cancel, &|_| {}).unwrap();
    finish(&mut reference);
    assert_eq!(completed.digest(), reference.generation().unwrap().digest());
    drop(reference);
    assert_eq!(
        raw_output(&f, &source, normal, &path, "recovered-census"),
        raw_output(&f, &source, normal, &control, "reference-census")
    );
}

#[test]
fn operational_relaxation_cannot_hide_changed_source_exponents_or_query_order() {
    let f = Fixture::new();
    let source = source(&f);
    let mut old = policy(1);
    old.operation_limits = None;
    let path = f.directory.join("source-identity");
    let cancel = AtomicBool::new(false);
    let mut session = Session::<1>::create(&source, old, &path, &cancel, &|_| {}).unwrap();
    session.step(&cancel, &|_| {}).unwrap();
    drop(session);
    let mut opened = old;
    opened.operation_limits = Some(OperationLimits::default());
    let before = tree(&path);
    for altered in ["exact-exponent", "indexed-exponent", "query-order"] {
        let mut changed = source.clone();
        match altered {
            "exact-exponent" => changed.matching.reduction_limits.exact_algebra.max_exponent -= 1,
            "indexed-exponent" => {
                changed
                    .matching
                    .reduction_limits
                    .indexed_algebra
                    .exact_algebra
                    .max_exponent -= 1
            }
            _ => {
                let mut queries: Value =
                    serde_json::from_str(&changed.matching.queries_json).unwrap();
                queries["queries"].as_array_mut().unwrap().reverse();
                changed.matching.queries_json = queries.to_string();
            }
        }
        assert!(
            Session::<1>::resume(&changed, opened, &path, &cancel, &|_| {}).is_err(),
            "{altered}"
        );
        assert_eq!(tree(&path), before, "{altered}");
    }
}
