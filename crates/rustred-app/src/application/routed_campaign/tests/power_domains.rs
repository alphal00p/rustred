use super::*;

fn graph_without_timings(mut value: Value) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("seconds");
                for child in o.values_mut() {
                    strip(child);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

#[test]
fn constrained_walk_translates_bounds_and_reaches_terminals_outside_entry_band() {
    let fixture = Fixture::new();
    let mut matching = match_request(&fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"powers-three-four", "owner":"1", "lower":[0], "upper":[null],
        "max_numerator_rank":11, "power_bounds":{"max_positive_power":4,
        "min_power_difference":3,"max_power_difference":4}}]})
    .to_string();
    let local = owner_domain_match_with_progress(matching.clone(), &AtomicBool::new(false), |_| {})
        .unwrap();
    assert!(local.all_queries_locally_applicable, "{}", local.document);
    for piece in local.document["queries"][0]["pieces"].as_array().unwrap() {
        assert!(piece["lower"][0].as_u64().unwrap() >= 2);
        assert!(piece["upper"][0].as_u64().unwrap() <= 3);
        assert_eq!(piece["power_bounds"]["min_power_difference"], 3);
    }
    let request = OwnerDomainWalkRequest::new(matching);
    let serial =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(serial.all_scheduled_domains_resolved, "{}", serial.document);
    assert_eq!(serial.document["frontiers"], 0);
    assert!(
        serial.document["domains"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["lower"][0] == 0
                && d["power_bounds"]["min_power_difference"]
                    .as_i64()
                    .is_some_and(|n| n < 3))
    );
    assert_eq!(serial.document["ibp_generation"], false);
    for workers in [2, 6] {
        let mut parallel = request.clone();
        parallel.workers = workers;
        let result =
            owner_domain_walk_with_progress(parallel, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(result.all_scheduled_domains_resolved, "{}", result.document);
        assert_eq!(
            graph_without_timings(result.document["domains"].clone()),
            graph_without_timings(serial.document["domains"].clone()),
            "{workers} workers"
        );
        for field in [
            "inputs",
            "scheduled_nodes",
            "events",
            "frontiers",
            "successors",
        ] {
            assert_eq!(
                result.document[field], serial.document[field],
                "{field}, workers={workers}"
            );
        }
    }
}

#[test]
fn constrained_routing_retains_a_and_difference_bounds_into_mapped_owner() {
    let fixture = noninvolutive_route_fixture();
    let mut request = route_walk_request(&fixture, 0);
    request.matching.queries_json =
        json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"correlated-route", "owner":"110", "lower":[2,3,0], "upper":[4,5,0],
        "max_numerator_rank":0, "power_bounds":{"max_positive_power":8,"min_power_difference":7}}]})
        .to_string();
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.all_scheduled_domains_resolved, "{}", result.document);
    let mapped = result.document["domains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["phase"] == "Apply" && d["owner"] == "011")
        .unwrap();
    assert_eq!(mapped["power_bounds"]["max_positive_power"], 8);
    assert_eq!(mapped["power_bounds"]["min_power_difference"], 7);
    assert_eq!(mapped["rank"], 0);
    assert_eq!(result.document["frontiers"], 0);
}

#[test]
fn empty_power_domain_does_not_create_a_gap_or_discard_valid_sibling() {
    let fixture = Fixture::new();
    let mut matching = match_request(&fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"empty", "owner":"1", "lower":[0], "upper":[null], "max_numerator_rank":0,
        "power_bounds":{"max_positive_power":0}},
        {"id":"valid", "owner":"1", "lower":[1], "upper":[2], "max_numerator_rank":0,
        "power_bounds":{"max_positive_power":3,"min_power_difference":2}}]})
    .to_string();
    let result =
        owner_domain_match_with_progress(matching, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.all_queries_locally_applicable, "{}", result.document);
    assert_eq!(result.document["queries"][0]["pieces"], json!([]));
    assert!(
        !result.document["queries"][1]["pieces"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        result.document["queries"][0]["stats"]["correlation_empty_cells"]
            .as_u64()
            .unwrap()
            > 0
    );
}
