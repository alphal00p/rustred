use super::*;

#[test]
fn rescue_appends_before_rebuilding_replay_lookup_watermarks() {
    let mut fixture = Fixture::new();
    fixture.request.epoch_rolling = true;
    fixture.request.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
        fixture.directory.0.clone(),
    ));
    fixture.save(3, 2);
    fixture.request.checkpoint.as_mut().unwrap().resume = true;
    let parent = crate::application::routed_campaign::walking::checkpoint::epoch_request_binding(
        &fixture.request,
    );
    fixture
        .request
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
        path: fixture.directory.0.join("amendment.json"),
        text:
            json!({"schema":crate::application::routed_campaign::walking::rescue::AMENDMENT_SCHEMA,
            "sequence":1,"parent":parent,"supersede":["q-1"],"queries":[
                {"id":"replacement","owner":"1","lower":[1],"upper":[1],"max_numerator_rank":2}
            ]})
            .to_string(),
    });
    let identity = fixture.identity();
    let mut restored = fixture.open().unwrap();
    assert_eq!(restored.replay.len(), 2);
    assert!(
        restored
            .state
            .in_flight
            .values()
            .all(|meta| meta.published_len == 3)
    );
    let sequences: Vec<_> = restored
        .replay
        .iter()
        .map(|job| (job.seq, job.v0))
        .collect();
    let mut saves = 0;
    super::super::rescue::apply(
        &mut restored,
        &identity,
        &fixture.reducer,
        16,
        || false,
        &mut |state, _| {
            saves += 1;
            assert_eq!(state.state.watermark(), 4);
            assert!(
                state
                    .state
                    .in_flight
                    .values()
                    .all(|meta| meta.published_len == 4)
            );
        },
    )
    .unwrap();
    assert_eq!(saves, 1);
    assert_eq!(
        restored
            .replay
            .iter()
            .map(|job| (job.seq, job.v0))
            .collect::<Vec<_>>(),
        sequences
    );
    let view = restored.state.store.snapshot(restored.state.k).unwrap();
    assert_eq!(view.published_len, 4);
    for job in &restored.replay {
        assert_eq!(
            restored.state.in_flight[&job.parent].published_len as usize,
            view.published_len
        );
    }
    drop(view);
    drop(restored);
    let reopened = fixture.open().unwrap();
    assert_eq!(reopened.replay.len(), 2);
    assert!(
        reopened
            .state
            .in_flight
            .values()
            .all(|meta| meta.published_len == 4)
    );
}

#[test]
fn rejected_rescue_batch_keeps_previous_generation_authoritative() {
    let mut fixture = Fixture::new();
    fixture.save(3, 0);
    let latest = fs::read(fixture.directory.0.join(publication::LATEST)).unwrap();
    fixture.request.max_domains = 4;
    let parent = crate::application::routed_campaign::walking::checkpoint::epoch_request_binding(
        &fixture.request,
    );
    fixture
        .request
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
        path: fixture.directory.0.join("amendment.json"),
        text:
            json!({"schema":crate::application::routed_campaign::walking::rescue::AMENDMENT_SCHEMA,
            "sequence":1,"parent":parent,"supersede":["q-1"],"queries":[
                {"id":"replacement","owner":"1","lower":[1],"upper":[1],"max_numerator_rank":2},
                {"id":"too-many","owner":"1","lower":[99],"upper":[99],"max_numerator_rank":2}
            ]})
            .to_string(),
    });
    let identity = fixture.identity();
    let mut restored = fixture.open().unwrap();
    let mut saved = false;
    let error = super::super::rescue::apply(
        &mut restored,
        &identity,
        &fixture.reducer,
        16,
        || false,
        &mut |_, _| saved = true,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("scheduled domain allowance"),
        "{error}"
    );
    assert!(!saved);
    assert_eq!(
        restored.state.watermark(),
        4,
        "failure exercised an actual partially prepared batch"
    );
    assert_eq!(restored.state.p0, 3);
    assert_eq!(
        fs::read(fixture.directory.0.join(publication::LATEST)).unwrap(),
        latest
    );
    assert!(!fixture.directory.0.join("epoch-poison").exists());
    drop(restored);
    drop(identity);
    fixture.request.amendments.clear();
    let restored = fixture.open().unwrap();
    assert_eq!(restored.state.watermark(), 3);
    assert!(restored.state.rescue.is_none());
    assert!(!restored.state.store.rescue_duplicates);
}

#[test]
fn required_query_cannot_be_superseded_at_rescue_identity_boundary() {
    let mut fixture = Fixture::new();
    let parent = crate::application::routed_campaign::walking::checkpoint::epoch_request_binding(
        &fixture.request,
    );
    fixture
        .request
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
        path: fixture.directory.0.join("amendment.json"),
        text:
            json!({"schema":crate::application::routed_campaign::walking::rescue::AMENDMENT_SCHEMA,
            "sequence":1,"parent":parent,"supersede":["q-0"],"queries":[]})
            .to_string(),
    });
    assert!(Identity::new(&fixture.request, &fixture.owners, &fixture.queries).is_err());
}

#[test]
fn cancelled_rescue_preparation_never_publishes_a_partial_batch() {
    let mut fixture = Fixture::new();
    fixture.save(3, 0);
    let latest = fs::read(fixture.directory.0.join(publication::LATEST)).unwrap();
    let parent = crate::application::routed_campaign::walking::checkpoint::epoch_request_binding(
        &fixture.request,
    );
    fixture
        .request
        .amendments
        .push(crate::OwnerDomainWalkAmendment {
        path: fixture.directory.0.join("amendment.json"),
        text:
            json!({"schema":crate::application::routed_campaign::walking::rescue::AMENDMENT_SCHEMA,
            "sequence":1,"parent":parent,"supersede":["q-1"],"queries":[
                {"id":"replacement","owner":"1","lower":[1],"upper":[1],"max_numerator_rank":2},
                {"id":"another","owner":"1","lower":[99],"upper":[99],"max_numerator_rank":2}
            ]})
            .to_string(),
    });
    let identity = fixture.identity();
    let mut saw_partial_preparation = false;
    for cutoff in 1..128 {
        let mut restored = fixture.open().unwrap();
        let mut checks = 0;
        let mut saved = false;
        let error = super::super::rescue::apply(
            &mut restored,
            &identity,
            &fixture.reducer,
            16,
            || {
                checks += 1;
                checks >= cutoff
            },
            &mut |_, _| saved = true,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Interrupted, "{error}");
        assert!(!saved);
        assert_eq!(
            fs::read(fixture.directory.0.join(publication::LATEST)).unwrap(),
            latest
        );
        if restored.state.watermark() > 3 {
            saw_partial_preparation = true;
            break;
        }
    }
    assert!(
        saw_partial_preparation,
        "cancelled after at least one appended ID"
    );
    drop(identity);
    fixture.request.amendments.clear();
    let restored = fixture.open().unwrap();
    assert_eq!(restored.state.watermark(), 3);
    assert!(restored.state.rescue.is_none());
}
