//! Required-scope continuation through the actual public CP6 entry points.
//! Rank stages enlarge the numerator domain of a two-loop pinched owner;
//! they reuse an unchanged generated rule bundle and append-only checkpoint.
use super::*;
use crate::{
    OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA, OwnerDomainWalkAmendment,
    OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyOptions,
    owner_domain_walk_verify_closure,
};

fn row(rank: u32) -> Value {
    json!({"id":format!("rank-{rank}"),"owner":"011","lower":[0,0,0],
        "upper":[null,0,0],"max_numerator_rank":rank})
}

fn queries(maximum: u32) -> String {
    json!({"schema":"rustred.owner-domain-queries.json.v2",
        "queries":(0..=maximum).map(row).collect::<Vec<_>>(),
        "query_roles":{"required":(0..=maximum).map(|r|format!("rank-{r}")).collect::<Vec<_>>(),
            "auxiliary":[]}})
    .to_string()
}

fn extension(rank: u32, parent: &str) -> OwnerDomainWalkAmendment {
    OwnerDomainWalkAmendment {
        path: format!("rank-{rank}.json").into(),
        text: json!({"schema":OWNER_DOMAIN_WALK_SCOPE_EXTENSION_SCHEMA,
            "sequence":rank,"parent":parent,"queries":[row(rank)],
            "query_roles":{"required":[format!("rank-{rank}")],"auxiliary":[]}})
        .to_string(),
    }
}

fn run(request: &OwnerDomainWalkRequest) -> Value {
    owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
        .unwrap()
        .document
}

fn verify(request: &OwnerDomainWalkRequest) -> Value {
    let mut options =
        OwnerDomainWalkVerifyOptions::new(&request.checkpoint.as_ref().unwrap().directory);
    options.require_closure = true;
    owner_domain_walk_verify_closure(request, &options, &AtomicBool::new(false), |_| {}).unwrap()
}

#[test]
fn required_scope_rank_zero_one_two_reuses_cp6_and_matches_one_shot() {
    let fixture = noninvolutive_route_fixture();
    let mut request = route_walk_request(&fixture, 0);
    request.matching.queries_json = queries(0);
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request.workers = 1;
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(
        fixture.directory.join("rank-stages"),
    ));
    let scalar = run(&request);
    assert_eq!(scalar["recursive_worklist_exhausted"], true, "{scalar}");
    assert_eq!(verify(&request)["verdict"], "PASS");
    let original_queries = request.matching.queries_json.clone();
    let original_rules = std::fs::read(fixture.directory.join("two_loop.rrbin")).unwrap();
    let original_prefix = scalar["initial_entry_domains_total"].clone();
    let mut previous_nodes = scalar["scheduled_nodes"].as_u64().unwrap();
    let mut previous_native = scalar["native_processed_nodes"].as_u64().unwrap();
    let mut parent = request.checkpoint_binding();
    request.checkpoint.as_mut().unwrap().resume = true;
    for rank in 1..=2 {
        let amendment = extension(rank, &parent);
        parent = amendment.digest();
        request.amendments.push(amendment);
        if rank == 1 {
            // Admit the enlarged required scope but stop before inspecting it.
            // AUTO cold verification must not mistake the closed scalar prefix
            // for closure of the newly required numerator domain.
            let mut stopped = request.clone();
            stopped.max_events = scalar["events"].as_u64().unwrap().max(1) as usize;
            let paused = run(&stopped);
            assert_eq!(paused["query_admission"]["required"], 2);
            assert_ne!(paused["recursive_worklist_exhausted"], true, "{paused}");
            let cold = verify(&stopped);
            assert_ne!(cold["verdict"], "PASS", "{cold}");
            assert_eq!(cold["physics_queries"]["total"], 2);
            assert!(cold["physics_queries"]["certified"].as_u64().unwrap() < 2);
        }
        let completed = run(&request);
        assert_eq!(
            completed["recursive_worklist_exhausted"], true,
            "{completed}"
        );
        assert_eq!(completed["required_queries_resolved"], true, "{completed}");
        assert_eq!(completed["query_admission"]["required"], rank + 1);
        assert_eq!(completed["query_admission"]["required_closed"], rank + 1);
        assert_eq!(completed["query_admission"]["original_required"], 1);
        assert_eq!(completed["query_admission"]["appended_required"], rank);
        assert_eq!(completed["initial_entry_domains_total"], original_prefix);
        assert!(completed["scheduled_nodes"].as_u64().unwrap() >= previous_nodes);
        assert!(completed["native_processed_nodes"].as_u64().unwrap() >= previous_native);
        previous_nodes = completed["scheduled_nodes"].as_u64().unwrap();
        previous_native = completed["native_processed_nodes"].as_u64().unwrap();
        let cold = verify(&request);
        assert_eq!(cold["verdict"], "PASS", "{cold}");
        assert_eq!(cold["physics_queries"]["total"], rank + 1);
        assert_eq!(cold["physics_queries"]["certified"], rank + 1);
        // Cold resume must not re-admit or inspect already completed natives.
        let resumed = run(&request);
        assert_eq!(resumed["scheduled_nodes"], completed["scheduled_nodes"]);
        assert_eq!(
            resumed["native_processed_nodes"],
            completed["native_processed_nodes"]
        );
        assert_eq!(request.matching.queries_json, original_queries);
    }
    assert_eq!(
        std::fs::read(fixture.directory.join("two_loop.rrbin")).unwrap(),
        original_rules
    );
    let mut one_shot = request.clone();
    one_shot.matching.queries_json = queries(2);
    one_shot.amendments.clear();
    one_shot.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(
        fixture.directory.join("rank-one-shot"),
    ));
    assert_eq!(run(&one_shot)["recursive_worklist_exhausted"], true);
    assert_eq!(verify(&one_shot)["verdict"], "PASS");

    // Recorded role/geometry bytes are immutable and all links are required.
    let mut omitted = request.clone();
    omitted.amendments.pop();
    assert!(owner_domain_walk_with_progress(omitted, &AtomicBool::new(false), |_| {}).is_err());
    let mut changed = request.clone();
    changed.amendments[0].text.push(' ');
    assert!(owner_domain_walk_with_progress(changed, &AtomicBool::new(false), |_| {}).is_err());
}
