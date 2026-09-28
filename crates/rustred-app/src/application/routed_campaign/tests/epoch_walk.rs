//! Publication policy `epoch` (walk semantics 3, W2 stage S2) end to end on
//! the one-axis fixture: drained and resolved, identical across widths, and
//! its final export certified by the closure verifier.
use super::*;
use crate::{
    OwnerDomainWalkCheckpointOptions, OwnerDomainWalkPublicationPolicy,
    OwnerDomainWalkSchedulingPolicy, OwnerDomainWalkVerifyOptions,
};

fn epoch_request(fixture: &Fixture, workers: usize) -> OwnerDomainWalkRequest {
    let mut matching = match_request(fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"narrow-a","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11},
        {"id":"narrow-b","owner":"1","lower":[3],"upper":[3],"max_numerator_rank":11},
        {"id":"whole-ray","owner":"1","lower":[0],"upper":[null],"max_numerator_rank":11},
        {"id":"duplicate","owner":"1","lower":[2],"upper":[2],"max_numerator_rank":11}
    ]})
    .to_string();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.publication_policy = OwnerDomainWalkPublicationPolicy::Epoch;
    request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
        lookahead: std::num::NonZeroUsize::new(8).unwrap(),
    };
    request.workers = workers;
    request
}

fn without_timing(mut value: Value) -> Value {
    fn visit(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.retain(|key, _| key != "seconds" && !key.ends_with("_seconds"));
                o.values_mut().for_each(visit);
            }
            Value::Array(a) => a.iter_mut().for_each(visit),
            _ => {}
        }
    }
    visit(&mut value);
    value
}

#[test]
fn epoch_walk_drains_resolved_and_is_identical_across_widths() {
    let fixture = Fixture::new();
    let mut baseline = None;
    for workers in [1, 2, 4] {
        if !crate::test_gates::workers_or_skip("epoch walk", workers) {
            continue;
        }
        let result = owner_domain_walk_with_progress(
            epoch_request(&fixture, workers),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        let document = result.into_document().unwrap();
        assert_eq!(document["status"], "locally_resolved", "{document}");
        assert_eq!(document["walk_semantics_version"], 3);
        assert_eq!(document["family_closure_claim"], false);
        assert_eq!(document["epoch"]["ledger6"]["pending"], 0);
        assert_eq!(document["epoch"]["certified"], true);
        // The duplicate query shares the first narrow domain's root.
        assert_eq!(
            document["inputs"][3]["domain"],
            document["inputs"][0]["domain"]
        );
        let records = document["domains"].as_array().unwrap();
        assert_eq!(
            records.len() as u64,
            document["scheduled_nodes"].as_u64().unwrap()
        );
        for record in records {
            assert_eq!(record["descendant_closed"], true, "{record}");
            if record["record_kind"] == "delegated_not_inspected" {
                assert!(record["representative_id"].as_u64() > record["id"].as_u64());
            } else {
                assert_eq!(record["accepted_events"], record["stats"]["events"]);
            }
        }
        let comparable = without_timing(json!({"domains":document["domains"],
            "inputs":document["inputs"],"epoch_ledger":document["epoch"]["ledger6"],
            "digests":[document["epoch"]["records_digest"],document["epoch"]["edge_digest"]]}));
        match &baseline {
            None => baseline = Some(comparable),
            Some(expected) => assert_eq!(&comparable, expected, "W{workers} differs from W1"),
        }
    }
}

#[test]
fn epoch_export_is_certified_by_the_closure_verifier_and_resume_is_refused() {
    let fixture = Fixture::new();
    let directory = fixture.directory.join("epoch-export");
    let mut request = epoch_request(&fixture, 1);
    request.checkpoint = Some(OwnerDomainWalkCheckpointOptions::new(&directory));
    let result =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.all_scheduled_domains_resolved, "{}", result.document);
    assert!(result.records.is_streamed());
    assert!(directory.join("epoch-export.json").is_file());
    let mut options = OwnerDomainWalkVerifyOptions::new(&directory);
    options.require_closure = true;
    let report = crate::owner_domain_walk_verify_closure(
        &request,
        &options,
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap();
    assert_eq!(report["verdict"], "PASS", "{report}");
    assert_eq!(
        report["roots_independently_verified"],
        report["roots_total"]
    );
    assert_eq!(report["checkpoint"]["publication_policy"], "epoch");
    assert_eq!(report["checkpoint"]["request_binding_matches"], true);
    // A second run into the same directory and a resume are refused.
    assert!(
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).is_err()
    );
    let mut resume = request;
    resume.checkpoint.as_mut().unwrap().resume = true;
    assert!(owner_domain_walk_with_progress(resume, &AtomicBool::new(false), |_| {}).is_err());
}
