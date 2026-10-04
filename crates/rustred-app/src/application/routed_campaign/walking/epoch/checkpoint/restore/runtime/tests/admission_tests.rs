use super::super::super::super::{
    metadata::{AdmissionFailure, AdmissionFailureKind, OwnedScalars},
    session, stop,
};
use super::super::{admission, controller};
use super::*;
use crate::application::routed_campaign::walking::epoch::{AdmissionError, admission as row};
use std::sync::atomic::AtomicBool;

fn pause() -> stop::Stop {
    stop::requested(&AtomicBool::new(true), None).unwrap()
}

fn bind_queries(fixture: &mut Fixture, declared: bool) {
    let mut document = json!({"schema":"rustred.owner-domain-queries.json.v2",
        "queries":fixture.queries.iter().map(|q|json!({"id":q.id,"owner":if q.owner[0] {"1"} else {"0"},
            "lower":q.lower,"upper":q.upper,"max_numerator_rank":q.rank})).collect::<Vec<_>>()});
    if declared {
        document["query_roles"] = json!({
        "required":fixture.queries.iter().filter(|q|!q.auxiliary).map(|q|&q.id).collect::<Vec<_>>(),
        "auxiliary":fixture.queries.iter().filter(|q|q.auxiliary).map(|q|&q.id).collect::<Vec<_>>()});
    }
    fixture.request.matching.queries_json = document.to_string();
    fixture.queries = crate::application::routed_campaign::matching::input::parse(
        &fixture.request.matching.queries_json,
        1,
        100,
        1 << 20,
    )
    .unwrap();
}

fn snapshot(restored: &Restored<1>) -> Value {
    json!({"rows":restored.roots.rows,"frontiers":restored.roots.frontiers,
        "images":restored.state.store.domains.iter().map(|image|image.expand()).collect::<Vec<_>>(),
        "ledger":restored.state.ledger.words(),"live":restored.state.live,"nodes":restored.state.nodes,
        "p0":restored.state.p0,"k":restored.state.k,"walk":restored.state.counters,
        "lookup":restored.state.lookup,"verify":restored.state.verify,
        "edges":restored.state.edges.log(),"records":restored.records.total(),
        "tracker":restored.state.tracker.node_flags().collect::<Vec<_>>(),
        "unavailable":restored.state.tracker.counters().unavailable})
}

fn finish(fixture: &Fixture, restored: &mut Restored<1>) {
    assert_eq!(
        admission::continue_admission(
            restored,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Ready
    );
}

#[test]
fn continuation_uses_rows_not_unique_ids_and_preserves_roles_and_session() {
    for declared in [false, true] {
        let mut fixture = Fixture::new();
        fixture.queries = [(2, Some(2)), (2, Some(2)), (0, None), (3, Some(3))]
            .into_iter()
            .enumerate()
            .map(|(i, (lo, hi))| Query {
                id: format!(
                    "{}-{i}",
                    if declared {
                        "declared"
                    } else {
                        "helper-but-required"
                    }
                ),
                auxiliary: declared && i == 1,
                role_declared: declared,
                owner: vec![true],
                lower: vec![lo],
                upper: vec![hi],
                rank: Some(11),
                powers: Default::default(),
            })
            .collect();
        bind_queries(&mut fixture, declared);
        let mut uninterrupted =
            admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
        finish(&fixture, &mut uninterrupted);
        let expected = snapshot(&uninterrupted);
        drop(uninterrupted);
        fixture.directory = Directory::new();
        let mut prefix =
            admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
        for after in [0, 2] {
            let mut polls = 0;
            assert_eq!(
                admission::continue_admission(
                    &mut prefix,
                    &fixture.identity(),
                    &fixture.reducer,
                    16,
                    || {
                        let stop = polls == after;
                        polls += 1;
                        stop.then(pause)
                    },
                    |_| {}
                )
                .unwrap(),
                admission::Outcome::Stopped(merge::StopReason::Paused)
            );
            assert_eq!(prefix.roots.rows.len(), after);
            assert_eq!(prefix.state.p0, u32::from(after != 0));
            assert!(prefix.state.tracker.counters().unavailable.is_none());
            assert!(matches!(
                prefix.dispatch.refill(&mut prefix.state, 16),
                Refill::Stalled
            ));
            assert!(
                controller::run_authorized(
                    &mut prefix,
                    &fixture.identity(),
                    16,
                    2,
                    MergeConfig {
                        frontier_stop: true,
                        lockstep: true,
                        g2: false,
                        finite_replay: None,
                    },
                    &|| panic!("authorize before complete"),
                    &|_, _| panic!("inspect before complete"),
                    || None,
                    |_, _| {}
                )
                .is_err()
            );
            drop(prefix);
            prefix = fixture.open().unwrap();
        }
        let session = prefix.dispatch.checkpoint_snapshot().session;
        finish(&fixture, &mut prefix);
        assert_eq!(prefix.dispatch.checkpoint_snapshot().session, session);
        assert_eq!(prefix.dispatch.checkpoint_snapshot().counter, 0);
        assert_eq!(snapshot(&prefix), expected);
        assert_eq!(prefix.roots.rows.len(), 4);
        assert_eq!(prefix.state.p0, 2);
        assert!(matches!(
            prefix.dispatch.refill(&mut prefix.state, 16),
            Refill::Jobs(_)
        ));
    }
}

#[test]
fn failure_diagnostic_survives_pause_retry_and_terminal_refusal() {
    let mut fixture = Fixture::new();
    fixture.request.max_domains = 1;
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    assert_eq!(
        admission::continue_admission(
            &mut prefix,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Stopped(merge::StopReason::DomainAllowance)
    );
    let failure = prefix.admission_failure.clone().unwrap();
    assert_eq!(failure.query_index, 1);
    assert_eq!(failure.message, "scheduled domain allowance");
    drop(prefix);
    let mut prefix = fixture.open().unwrap();
    assert_eq!(
        admission::continue_admission(
            &mut prefix,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || Some(pause()),
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Stopped(merge::StopReason::Paused)
    );
    drop(prefix);
    let prefix = fixture.open().unwrap();
    assert_eq!(prefix.admission_failure, Some(failure));
    drop(prefix);
    fixture.request.max_domains = 100;
    let mut prefix = fixture.open().unwrap();
    finish(&fixture, &mut prefix);
    assert!(prefix.admission_failure.is_none());
    assert_eq!(prefix.roots.rows.len(), 3);
    drop(prefix);

    fixture.directory = Directory::new();
    fixture.queries[1].lower = vec![70_000];
    fixture.queries[1].upper = vec![Some(70_000)];
    bind_queries(&mut fixture, true);
    let mut refused = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    assert_eq!(
        admission::continue_admission(
            &mut refused,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Stopped(merge::StopReason::ErrorStop)
    );
    let failure = refused.admission_failure.clone().unwrap();
    assert_eq!(failure.kind, AdmissionFailureKind::Refused);
    assert_eq!(failure.query_index, 1);
    drop(refused);
    let mut refused = fixture.open().unwrap();
    assert_eq!(
        admission::continue_with(
            &mut refused,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {},
            |_, _, _, _, _| panic!("terminal refusal must not retry")
        )
        .unwrap(),
        admission::Outcome::Stopped(merge::StopReason::ErrorStop)
    );
    assert_eq!(refused.admission_failure, Some(failure));
    assert_eq!(refused.roots.rows.len(), 1);
}

#[test]
fn admission_allocation_retry_preserves_prefix_and_real_monitor_failure() {
    let fixture = Fixture::new();
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    prefix
        .state
        .tracker
        .disable("test genuine monitor allocation failure");
    let outcome = admission::continue_with(
        &mut prefix,
        &fixture.identity(),
        &fixture.reducer,
        16,
        || None,
        |_| {},
        |state, query, phase, rows, frontiers| {
            row::one_with(state, query, phase, rows, frontiers, || {
                Err("injected row allocation")
            })
        },
    )
    .unwrap();
    assert_eq!(
        outcome,
        admission::Outcome::Stopped(merge::StopReason::RamGuard)
    );
    assert!(prefix.roots.rows.is_empty());
    assert_eq!(prefix.state.p0, 0);
    assert_eq!(
        prefix.admission_failure.as_ref().unwrap().message,
        "injected row allocation"
    );
    drop(prefix);
    let mut prefix = fixture.open().unwrap();
    finish(&fixture, &mut prefix);
    assert_eq!(
        prefix.state.tracker.counters().unavailable.as_deref(),
        Some("test genuine monitor allocation failure")
    );
    assert!(prefix.admission_failure.is_none());
}

#[test]
fn source_frontiers_keep_counts_monitor_reason_and_unprocessed_cap_row() {
    let mut fixture = Fixture::with_source_condition(true);
    assert!(fixture.reducer.domain_routing_requires_source_conditions());
    fixture.request.route_domain_overcover = true;
    fixture.request.max_frontiers = 1;
    fixture.queries[1].owner = vec![false];
    fixture.queries[2].owner = vec![false];
    bind_queries(&mut fixture, true);
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    let mut polls = 0;
    admission::continue_admission(
        &mut prefix,
        &fixture.identity(),
        &fixture.reducer,
        16,
        || {
            let stop = polls == 2;
            polls += 1;
            stop.then(pause)
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(prefix.roots.rows.len(), 2);
    assert_eq!(prefix.state.p0, 1);
    assert_eq!(prefix.roots.frontiers.len(), 1);
    assert_eq!(prefix.state.counters.frontiers, 1);
    assert_eq!(prefix.roots.rows[1]["domain"], Value::Null);
    assert_eq!(prefix.roots.rows[1]["source_validity_unresolved"], true);
    let reason = prefix.state.tracker.counters().unavailable.unwrap();
    drop(prefix);
    let mut prefix = fixture.open().unwrap();
    assert_eq!(
        admission::continue_admission(
            &mut prefix,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {}
        )
        .unwrap(),
        admission::Outcome::Stopped(merge::StopReason::FrontierAllowance)
    );
    assert_eq!(prefix.roots.rows.len(), 2);
    assert_eq!(prefix.admission_failure.as_ref().unwrap().query_index, 2);
    drop(prefix);
    fixture.request.max_frontiers = 2;
    let mut prefix = fixture.open().unwrap();
    finish(&fixture, &mut prefix);
    assert_eq!(prefix.roots.rows.len(), 3);
    assert_eq!(prefix.roots.frontiers.len(), 2);
    assert_eq!(prefix.state.counters.frontiers, 2);
    assert_eq!(prefix.state.p0, 1);
    assert_eq!(prefix.state.tracker.counters().unavailable, Some(reason));
    for (policy, expected) in [(true, Some(merge::StopReason::FrontierStop)), (false, None)] {
        assert_eq!(
            controller::initial_stop(
                MergeConfig {
                    frontier_stop: policy,
                    lockstep: true,
                    g2: false,
                    finite_replay: None,
                },
                prefix.roots.frontiers.len()
            ),
            expected
        );
    }
}

#[test]
fn missing_owner_route_prefix_roundtrips_before_readiness() {
    let mut fixture = Fixture::new();
    assert!(!fixture.reducer.domain_routing_requires_source_conditions());
    fixture.request.route_domain_overcover = true;
    fixture.queries[0].owner = vec![false];
    bind_queries(&mut fixture, true);
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    let mut polls = 0;
    admission::continue_admission(
        &mut prefix,
        &fixture.identity(),
        &fixture.reducer,
        16,
        || {
            let stop = polls == 1;
            polls += 1;
            stop.then(pause)
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(prefix.state.store.domains[0].phase(), Phase::Route);
    assert_eq!(prefix.roots.rows.len(), 1);
    assert_eq!(prefix.state.p0, 1);
    assert_eq!(prefix.roots.rows[0]["role"], "required");
    assert_eq!(prefix.roots.rows[0]["role_declared"], true);
    assert!(prefix.roots.frontiers.is_empty());
    assert_eq!(prefix.state.counters.dispatched, 0);
    assert!(matches!(
        prefix.dispatch.refill(&mut prefix.state, 16),
        Refill::Stalled
    ));
    let expected = snapshot(&prefix);
    drop(prefix);
    let mut prefix = fixture.open().unwrap();
    assert_eq!(snapshot(&prefix), expected);
    finish(&fixture, &mut prefix);
    assert_eq!(prefix.roots.rows.len(), 3);
    assert_eq!(prefix.state.store.domains[0].phase(), Phase::Route);
    assert_eq!(prefix.state.counters.dispatched, 0);
}

#[test]
fn initial_overlap_requires_completed_readiness_and_keeps_exact_membership() {
    use crate::application::routed_campaign::walking::initial_overlap::{
        InitialOverlapBuildStatus, InitialOverlapIndex,
    };
    let mut fixture = Fixture::new();
    fixture.request.reuse_initial_d_bands = true;
    let cancel = AtomicBool::new(false);
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    assert!(admission::initial_overlap(&prefix, &fixture.request, &cancel).is_err());
    let mut polls = 0;
    admission::continue_admission(
        &mut prefix,
        &fixture.identity(),
        &fixture.reducer,
        16,
        || {
            let stop = polls == 3;
            polls += 1;
            stop.then(pause)
        },
        |_| {},
    )
    .unwrap();
    assert_eq!(prefix.roots.rows.len(), 3);
    assert!(matches!(prefix.admission, Admission::InProgress));
    assert!(admission::initial_overlap(&prefix, &fixture.request, &cancel).is_err());
    finish(&fixture, &mut prefix);
    let domains: Vec<_> = prefix
        .state
        .store
        .domains
        .iter()
        .map(|image| Arc::new(image.expand()))
        .collect();
    let expected = InitialOverlapIndex::from_initial(&domains, &cancel);
    let index = admission::initial_overlap(&prefix, &fixture.request, &cancel).unwrap();
    assert_eq!(index.build_report(), expected.build_report());
    assert_eq!(index.build_report().total_initial, 3);
    assert_eq!(index.build_report().retained_membership, 3);
    assert_eq!(index.build_report().usable_anchors, 3);
    for domain in &domains {
        assert!(index.plan(domain, &cancel).is_none());
    }
    let query = Domain {
        phase: Phase::Apply,
        owner: [true],
        lower: vec![0],
        upper: vec![Some(2)],
        rank: Some(2),
        powers: Default::default(),
    };
    let actual = index.plan(&query, &cancel).unwrap();
    assert_eq!(actual.scope, expected.plan(&query, &cancel).unwrap().scope);
    assert_eq!(actual.scope.anchor_id, 2);
    cancel.store(true, std::sync::atomic::Ordering::Release);
    assert_eq!(
        admission::initial_overlap(&prefix, &fixture.request, &cancel)
            .err()
            .unwrap()
            .kind(),
        io::ErrorKind::Interrupted
    );
    fixture.request.reuse_initial_d_bands = false;
    assert_eq!(
        admission::initial_overlap(&prefix, &fixture.request, &cancel)
            .unwrap()
            .build_report()
            .status,
        InitialOverlapBuildStatus::NotRequested
    );
    assert_eq!(prefix.state.counters.dispatched, 0);
    assert_eq!(prefix.dispatch.checkpoint_snapshot().counter, 0);
}

#[test]
fn admission_c5_and_failed_publication_do_not_create_new_authority() {
    let fixture = Fixture::new();
    let mut prefix = admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
    admission::continue_admission(
        &mut prefix,
        &fixture.identity(),
        &fixture.reducer,
        16,
        || Some(pause()),
        |_| {},
    )
    .unwrap();
    let latest = fs::read(fixture.directory.0.join(publication::LATEST)).unwrap();
    prefix
        .publisher
        .fail_at(publication::FailPoint::BeforeLatest);
    assert!(
        admission::continue_admission(
            &mut prefix,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || Some(pause()),
            |_| {}
        )
        .is_err()
    );
    assert_eq!(
        fs::read(fixture.directory.0.join(publication::LATEST)).unwrap(),
        latest
    );
    drop(prefix);
    let mut prefix = fixture.open().unwrap();
    assert!(
        admission::continue_with(
            &mut prefix,
            &fixture.identity(),
            &fixture.reducer,
            16,
            || None,
            |_| {},
            |_, _, _, _, _| Err(AdmissionError::Internal(
                "injected admission inconsistency".into()
            ))
        )
        .is_err()
    );
    assert!(prefix.state.poisoned);
    assert_eq!(
        fs::read(fixture.directory.0.join(publication::LATEST)).unwrap(),
        latest
    );
    drop(prefix);
    assert!(fixture.open().is_err());
}

fn mutate_file(fixture: &Fixture, key: &str, change: impl FnOnce(&mut Vec<u8>)) {
    let mut manifest =
        publication::read_manifest(&fixture.directory.0.join(publication::LATEST)).unwrap();
    let file = manifest
        .files
        .iter_mut()
        .find(|file| file.key == key)
        .unwrap();
    let path = fixture.directory.0.join(&file.file);
    let mut bytes = fs::read(&path).unwrap();
    change(&mut bytes);
    fs::write(path, &bytes).unwrap();
    file.bytes = bytes.len() as u64;
    file.blake3 = *blake3::hash(&bytes).as_bytes();
    let digest = *blake3::hash(&serde_json::to_vec(&manifest).unwrap()).as_bytes();
    fs::write(
        fixture.directory.0.join(publication::LATEST),
        serde_json::to_vec(&json!({"manifest":manifest,"blake3":digest})).unwrap(),
    )
    .unwrap();
}

#[test]
fn old_private_scalars_and_issued_input_prefix_are_refused_before_session_adoption() {
    for old_version in [false, true] {
        let fixture = Fixture::new();
        let mut prefix =
            admission::fresh(fixture.directory.0.clone(), &fixture.identity()).unwrap();
        for _ in 0..2 {
            admission::continue_admission(
                &mut prefix,
                &fixture.identity(),
                &fixture.reducer,
                16,
                || Some(pause()),
                |_| {},
            )
            .unwrap();
        }
        drop(prefix);
        if old_version {
            mutate_file(&fixture, "meta", |bytes| {
                let mut value: Value = serde_json::from_slice(bytes).unwrap();
                value["schema"] = 1.into();
                *bytes = serde_json::to_vec(&value).unwrap();
            });
        } else {
            // Section header28; k,p0,B,session each8; issuance counter follows.
            mutate_file(
                &fixture,
                &format!(
                    "state-{}",
                    super::super::super::super::Section::Dispatch as u32
                ),
                |bytes| bytes[60..68].copy_from_slice(&1u64.to_le_bytes()),
            );
        }
        let before = session::read(&fixture.directory.0).unwrap();
        let error = fixture
            .open()
            .err()
            .expect("malformed input prefix must not adopt a session");
        if old_version {
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        }
        assert_eq!(session::read(&fixture.directory.0).unwrap(), before);
    }
}

#[test]
fn failure_metadata_rejects_wrong_index_kind_size_and_complete_state() {
    let fixture = Fixture::new();
    fixture.save(1, 0);
    let manifest =
        publication::read_manifest(&fixture.directory.0.join(publication::LATEST)).unwrap();
    let file = manifest
        .files
        .iter()
        .find(|file| file.key == "meta")
        .unwrap();
    let bytes = fs::read(fixture.directory.0.join(&file.file)).unwrap();
    for bad in 0..5 {
        let mut saved: OwnedScalars = serde_json::from_slice(&bytes).unwrap();
        saved.stop_reason = Some("domain_allowance".into());
        saved.admission_failure = Some(AdmissionFailure {
            query_index: 1,
            kind: AdmissionFailureKind::DomainAllowance,
            message: "scheduled domain allowance".into(),
        });
        assert!(fixture.identity().validate_saved(&saved, 16).is_ok());
        match bad {
            0 => saved.admission_failure.as_mut().unwrap().query_index = 0,
            1 => saved.admission_failure.as_mut().unwrap().kind = AdmissionFailureKind::Refused,
            2 => saved.admission_failure.as_mut().unwrap().message = "x".repeat(4097),
            3 => saved.initial_admission = Admission::Complete,
            _ => saved.admission_failure = None,
        }
        if bad == 4 {
            saved.stop_reason = Some("error_stop".into());
        }
        assert!(fixture.identity().validate_saved(&saved, 16).is_err());
    }
}
