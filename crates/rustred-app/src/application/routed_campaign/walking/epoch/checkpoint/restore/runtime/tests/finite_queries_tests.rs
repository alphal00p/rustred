//! Native finite-v3 lifecycle and exact input-row binding, no new IBP method.
use super::*;
use crate::application::routed_campaign::walking::epoch::inspector::{Context, inspect_job};
use crate::application::routed_campaign::walking::{
    finite_replay::{Account, INITIAL_QUERIES_VERSION, InitialBindings},
    initial_overlap::InitialOverlapIndex,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.request.epoch_rolling = true;
    f.request.finite_replay_initial_queries = true;
    f.request.finite_replay = Some(super::finite_replay_tests::limits());
    f.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(&f.directory.0));
    f.request.workers = 2;
    f
}
fn run(
    f: &Fixture,
    cancel: &AtomicBool,
    observer: &impl Fn(Value),
) -> crate::OwnerDomainWalkResult {
    super::super::public::run(
        &f.request,
        &f.reducer,
        &f.owners,
        &f.queries,
        16,
        Instant::now(),
        0.0,
        cancel,
        observer,
    )
    .unwrap()
}
fn bound(f: &Fixture, restored: &Restored<1>) -> InitialBindings<1> {
    InitialBindings::build(&f.reducer, &f.request, &restored.roots.rows, |id| {
        restored
            .state
            .store
            .domains
            .get(id as usize)
            .map(|image| image.expand())
    })
    .unwrap()
}
fn rewrite_queries(f: &mut Fixture) {
    f.request.matching.queries_json=json!({"schema":"rustred.owner-domain-queries.json.v2",
        "queries":f.queries.iter().map(|q|json!({"id":q.id,"owner":"1","lower":q.lower,"upper":q.upper,
            "max_numerator_rank":q.rank,"power_bounds":{"max_positive_power":q.powers.max_positive_power,
            "min_power_difference":q.powers.min_power_difference,"max_power_difference":q.powers.max_power_difference}})).collect::<Vec<_>>(),
        "query_roles":{"required":f.queries.iter().filter(|q|!q.auxiliary).map(|q|&q.id).collect::<Vec<_>>(),
            "auxiliary":f.queries.iter().filter(|q|q.auxiliary).map(|q|&q.id).collect::<Vec<_>>()}}).to_string();
}

#[test]
fn finite_queries_multiple_outstanding_refuses_before_adopting_a_session() {
    let f = fixture();
    // Deliberately bypass v3 dispatch's gate to model malformed saved input.
    f.save(3, 2);
    let paths = [
        f.directory.0.join("epoch-session.bin"),
        f.directory.0.join(publication::LATEST),
    ];
    let before = paths
        .iter()
        .map(fs::read)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let error = match f.open() {
        Ok(_) => panic!("adopted two outstanding finite queries"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("multiple outstanding initial queries"),
        "{error}"
    );
    assert_eq!(
        paths
            .iter()
            .map(fs::read)
            .collect::<Result<Vec<_>, _>>()
            .unwrap(),
        before
    );
}

#[test]
fn finite_queries_restore_rejects_identity_role_and_geometry_mutations() {
    let f = fixture();
    f.save(3, 0);
    let restored = f.open().unwrap();
    let bindings = bound(&f, &restored);
    assert_eq!(bindings.len(), 3);
    for field in ["id", "role", "role_declared", "amendment", "domain"] {
        let mut rows = restored.roots.rows.clone();
        rows[0][field] = match field {
            "role_declared" => false.into(),
            "amendment" | "domain" => 1.into(),
            _ => "changed".into(),
        };
        assert!(
            InitialBindings::build(&f.reducer, &f.request, &rows, |id| restored
                .state
                .store
                .domains
                .get(id as usize)
                .map(|image| image.expand()))
            .is_err(),
            "{field}"
        );
    }
    assert!(
        InitialBindings::build(&f.reducer, &f.request, &restored.roots.rows, |id| {
            let mut domain = restored.state.store.domains.get(id as usize)?.expand();
            domain.lower[0] = 99;
            domain.upper[0] = Some(99);
            Some(domain)
        })
        .is_err()
    );
}

#[test]
fn finite_queries_containment_aliases_are_valid_but_do_not_authorize_broader_descendants() {
    let mut f = fixture();
    f.queries.truncate(2);
    f.queries[0].lower = vec![0];
    f.queries[0].upper = vec![Some(3)];
    rewrite_queries(&mut f);
    let parent =
        crate::application::routed_campaign::walking::checkpoint::epoch_request_binding(&f.request);
    f.request.amendments.push(crate::OwnerDomainWalkAmendment {
        path:f.directory.0.join("narrow-alias.json"),text:json!({
            "schema":crate::application::routed_campaign::walking::rescue::SCOPE_EXTENSION_SCHEMA,
            "sequence":1,"parent":parent,"queries":[{"id":"appended","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":2}],
            "query_roles":{"required":["appended"],"auxiliary":[]}
        }).to_string(),
    });
    f.request.checkpoint.as_mut().unwrap().resume = true;
    let mut state = EpochState::<1>::new(100, 100, 100);
    let mut broad = Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![0],
        upper: vec![Some(3)],
        rank: Some(2),
        powers: Default::default(),
    };
    assert_eq!(admit_initial(&mut state, &broad).unwrap(), 0);
    state.p0 = 1; // Freeze the protected original prefix before the larger image.
    broad.upper[0] = Some(5);
    assert_eq!(admit_initial(&mut state, &broad).unwrap(), 1);
    assert_eq!(state.p0, 1);
    let rows = vec![
        json!({"id":"q-0","domain":0,"role":"required","role_declared":true}),
        json!({"id":"q-1","domain":0,"role":"auxiliary","role_declared":true}),
        json!({"id":"appended","domain":1,"role":"required","role_declared":true,"amendment":1}),
    ];
    let bindings = InitialBindings::build(&f.reducer, &f.request, &rows, |id| {
        state
            .store
            .domains
            .get(id as usize)
            .map(|image| image.expand())
    })
    .unwrap();
    assert!(bindings.contains(0));
    assert!(!bindings.contains(1));
    assert_eq!(bindings.len(), 1);
    let exact=crate::application::routed_campaign::walking::epoch::checkpoint::restore::roots::exact_input_ids(
        &f.identity(),&f.reducer,&state.store,&rows).unwrap();
    assert_eq!(exact, std::collections::BTreeSet::from([0]));
}

#[test]
fn finite_queries_nonzero_attempts_and_available_g2_do_not_force_symbolic_fallback() {
    let mut f = fixture();
    f.request.reuse_initial_d_bands = true;
    for (index, min_d) in [(0, 2), (2, 1)] {
        f.queries[index].lower = vec![0];
        f.queries[index].upper = vec![None];
        f.queries[index].powers = rustred::solver::DomainPowerBounds {
            max_positive_power: Some(3),
            min_power_difference: Some(min_d),
            max_power_difference: Some(3),
        };
    }
    // Keep all three rows admitting distinct roots in Fixture::save; its
    // tracker is intentionally initialized for three domains, not aliases.
    f.queries[1].lower = vec![4];
    f.queries[1].upper = vec![Some(4)];
    f.request.matching.queries_json=json!({"schema":"rustred.owner-domain-queries.json.v2",
        "queries":f.queries.iter().map(|q|json!({"id":q.id,"owner":"1","lower":q.lower,"upper":q.upper,
            "max_numerator_rank":q.rank,"power_bounds":{"max_positive_power":q.powers.max_positive_power,
            "min_power_difference":q.powers.min_power_difference,"max_power_difference":q.powers.max_power_difference}})).collect::<Vec<_>>(),
        "query_roles":{"required":["q-0","q-2"],"auxiliary":["q-1"]}}).to_string();
    f.save(3, 0);
    let restored = f.open().unwrap();
    let mut account = Account::default();
    account
        .install_bindings(Arc::new(bound(&f, &restored)))
        .unwrap();
    let g2 = crate::application::routed_campaign::walking::g2::Store::new(None, 1, 1);
    let cancel = AtomicBool::new(false);
    let overlap = InitialOverlapIndex::from_initial(
        &[Arc::new(restored.state.store.domains[0].expand())],
        &cancel,
    );
    let j = crate::application::routed_campaign::walking::epoch::job::Job {
        seq: 1,
        parent: 2,
        v0: 0,
        attempts: 1,
        flags: 0,
        image: restored.state.store.domains[2],
    };
    assert!(g2.eligible(2, &j.image.expand()));
    assert!(
        overlap.plan(&j.image.expand(), &cancel).is_some(),
        "real available high-D overlap"
    );
    let result = JobResult::<1>::decode(&inspect_job(
        &Context {
            reducer: &f.reducer,
            request: &f.request,
            overlap: &overlap,
            cancellation: &cancel,
            g2: Some(&g2),
            finite_account: Some(&account),
        },
        &j.encode(),
    ))
    .unwrap();
    assert_eq!(result.kind, NativeKind::FiniteReplay);
    assert_eq!(
        result.finite_replay.unwrap().version,
        INITIAL_QUERIES_VERSION
    );
    assert!(result.scope.is_none() && result.g2.is_none());
    assert_eq!(account.report()["completed_cases"], 1);
}

#[test]
fn finite_queries_completed_cases_survive_stop_resume_and_append_scope() {
    const TEST: &str = "finite initial queries public checkpoint lifecycle";
    if !crate::test_gates::workers_or_skip(TEST, 2) {
        return;
    }
    struct Unthrottled;
    impl Drop for Unthrottled {
        fn drop(&mut self) {
            super::super::public::TEST_UNTHROTTLED_PROGRESS.with(|flag| flag.set(false));
        }
    }
    super::super::public::TEST_UNTHROTTLED_PROGRESS.with(|flag| assert!(!flag.replace(true)));
    let _unthrottled = Unthrottled;
    let mut f = fixture();
    f.request.g2_residual_anchors = crate::OwnerDomainWalkG2ResidualAnchors::Union;
    f.request.reuse_initial_d_bands = true;
    let cancel = AtomicBool::new(false);
    let first = run(&f, &cancel, &|event| {
        if event["event"] == "epoch_heartbeat"
            && event["completed_nodes"].as_u64().is_some_and(|n| n >= 1)
        {
            cancel.store(true, Ordering::Release);
        }
    });
    assert_eq!(first.document["recursive_worklist_exhausted"], false);
    assert_eq!(first.document["resume_supported"], true);
    let completed = first.document["completed_nodes"].as_u64().unwrap();
    assert_eq!(completed, 1, "stop after the first atomic finite commit");
    f.request.checkpoint.as_mut().unwrap().resume = true;
    let second = run(&f, &AtomicBool::new(false), &|_| {});
    assert_eq!(second.document["recursive_worklist_exhausted"], true);
    assert_eq!(second.document["finite_replay"]["attempts"], 2);
    let restored = f.open().unwrap();
    assert_eq!(restored.state.p0, 3);
    assert_eq!(restored.state.pending_or_reserved(), 0);
    drop(restored);
    let parent =
        crate::application::routed_campaign::walking::checkpoint::epoch_request_binding(&f.request);
    f.request.amendments.push(crate::OwnerDomainWalkAmendment {
        path:f.directory.0.join("extension.json"),text:json!({
            "schema":crate::application::routed_campaign::walking::rescue::SCOPE_EXTENSION_SCHEMA,
            "sequence":1,"parent":parent,"queries":[{"id":"extended","owner":"1","lower":[3],"upper":[3],"max_numerator_rank":2}],
            "query_roles":{"required":["extended"],"auxiliary":[]}
        }).to_string(),
    });
    let third = run(&f, &AtomicBool::new(false), &|_| {});
    assert_eq!(third.document["recursive_worklist_exhausted"], true);
    assert_eq!(third.document["finite_replay"]["attempts"], 1);
    assert_eq!(third.document["finite_replay"]["completed_cases"], 1);
    let mut restored = f.open().unwrap();
    assert_eq!(restored.state.p0, 3);
    assert_eq!(restored.roots.rows.len(), 4);
    assert_eq!(bound(&f, &restored).len(), 4);
    let mut recipes = Vec::new();
    restored
        .records
        .files()
        .for_each_record(|record| {
            let value = record.project()?;
            if value["record_kind"] == "finite_replay_summary" {
                recipes.push(value);
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(recipes.len(), 4);
    assert!(
        recipes
            .iter()
            .all(|record| record["finite_replay_recipe"]["version"] == 3)
    );
}
