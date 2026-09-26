//! Checkpointed walks keep their records in the checkpoint's record sidecar;
//! the report streams them back and must equal the in-memory report.
use super::*;

fn without_timings(mut value: Value) -> Value {
    fn strip(v: &mut Value) {
        match v {
            Value::Object(o) => {
                o.remove("seconds");
                o.remove("physical_seconds_sum");
                // Ready replay prefixes exist only when checkpointing.
                o.remove("accepted_events");
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
fn streaming_result_json_equals_materialized_document() {
    let fixture = Fixture::new();
    let mut matching = match_request(&fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v2", "queries":[
        {"id":"powers-three-four", "owner":"1", "lower":[0], "upper":[null],
        "max_numerator_rank":11, "power_bounds":{"max_positive_power":4,
        "min_power_difference":3,"max_power_difference":4}}]})
    .to_string();
    for (label, ready) in [("ordered", false), ("ready", true)] {
        let mut request = OwnerDomainWalkRequest::new(matching.clone());
        request.scheduling_policy = OwnerDomainWalkSchedulingPolicy::TransferUnreserved {
            lookahead: std::num::NonZeroUsize::new(2).unwrap(),
        };
        if ready {
            request.publication_policy = OwnerDomainWalkPublicationPolicy::Ready;
        }
        let inline =
            owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
                .unwrap();
        assert!(!inline.records.is_streamed());
        assert!(inline.document["domains"].as_array().unwrap().len() > 1);
        let mut checkpointed = request;
        checkpointed.checkpoint = Some(crate::OwnerDomainWalkCheckpointOptions::new(
            fixture.directory.join(format!("checkpoint-{label}")),
        ));
        let streamed =
            owner_domain_walk_with_progress(checkpointed, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(streamed.records.is_streamed(), "{label}");
        assert!(streamed.document["domains"].is_null());
        assert_eq!(
            streamed.all_scheduled_domains_resolved,
            inline.all_scheduled_domains_resolved
        );
        let mut bytes = Vec::new();
        streamed.write_json(&mut bytes).unwrap();
        let document = streamed.clone().into_document().unwrap();
        // The streaming writer is the materialized pretty writer, byte for byte.
        assert_eq!(
            bytes,
            serde_json::to_vec_pretty(&document).unwrap(),
            "{label}"
        );
        let domains = &document["domains"];
        assert_eq!(
            domains.as_array().unwrap().len(),
            streamed.document["committed_domains"].as_u64().unwrap() as usize
        );
        assert_eq!(
            without_timings(domains.clone()),
            without_timings(inline.document["domains"].clone()),
            "{label}"
        );
        for field in [
            "scheduled_nodes",
            "completed_nodes",
            "events",
            "successors",
            "frontiers",
            "delegation",
            "descendant_closure",
        ] {
            let mut a = document[field].clone();
            let mut b = inline.document[field].clone();
            if field == "descendant_closure" {
                for key in [
                    "snapshot_age_seconds",
                    "last_refresh_seconds",
                    "refresh_seconds",
                    "refresh_count",
                    "snapshot_revision",
                    "snapshot_stale",
                    "retained_storage_estimate_bytes",
                ] {
                    a.as_object_mut().unwrap().remove(key);
                    b.as_object_mut().unwrap().remove(key);
                }
            }
            assert_eq!(a, b, "{label}: {field}");
        }
    }
}
