use super::*;
use crate::{FamilyCandidatesRequest, family_candidates, inspect_generated_candidate_bundle};
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn shared_snapshot_exposes_first_failure_while_native_calls_drain() {
    let snapshot = CandidateRoutedCampaignSnapshot::<3> {
        active_nodes: 2,
        failed_nodes: 1,
        first_failure: Some(CandidateRoutedCampaignFailure::Trace(
            rustred::solver::CandidateRoutedError::ResourceLimit {
                resource: "transport endpoints",
                requested: 101,
                limit: 100,
            },
        )),
        first_failure_work: Some(CandidateRoutedWork::Apply {
            owner_sector: [true, false, true],
            target: rustred::family::IntegralKey::try_new([2, -1, 3]).unwrap(),
        }),
        ..Default::default()
    };
    let value = snapshot_json(&snapshot);
    assert_eq!(value["first_failure"]["kind"], "trace");
    let detail = value["first_failure"]["detail"].as_str().unwrap();
    assert!(detail.contains("transport endpoints"));
    assert!(detail.contains("requested: 101"));
    assert!(detail.contains("limit: 100"));
    assert_eq!(value["first_failure_work"]["phase"], "apply");
    assert_eq!(value["first_failure_work"]["owner_mask"], "101");
    assert_eq!(value["first_failure_work"]["target"], json!([2, -1, 3]));
    assert_eq!(value["active_nodes"], 2);
    assert_eq!(value["finished"], false);
    assert_eq!(value["target_completion_known"], false);
    let empty = snapshot_json(&CandidateRoutedCampaignSnapshot::<3>::default());
    assert!(empty["first_failure"].is_null());
    assert!(empty["first_failure_work"].is_null());
}

const K1: &str = r#"
schema="rustred.project.toml.v1"
[family]
name="generic_shared_campaign_test"
loop_momenta=["q"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P"
expression="q^2-1"
[target]
powers=[1]
"#;

struct Fixture {
    directory: PathBuf,
    request: RoutedCampaignRequest,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "rustred-routed-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut request = FamilyCandidatesRequest::new(K1);
        request.numerical_depth = 0;
        request.max_numerator_rank = Some(2);
        let bundle = family_candidates(request).unwrap();
        let inspection =
            inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
        let path = directory.join("owner.rrbin");
        std::fs::write(&path, bundle.bundle()).unwrap();
        let selection = json!({"family_fingerprint":inspection.family_fingerprint,
            "owners":[{"path":"owner.rrbin","bytes":bundle.bundle().len(),"mask":"1"}],
            "initial_frontier_routes":[]});
        let mut request = RoutedCampaignRequest::new(selection.to_string(), "2\n3\n2\n".into());
        request.owner_base = directory.clone();
        Self { directory, request }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn shared_owner_campaign_is_generic_deduplicated_and_never_claims_family_closure() {
    let fixture = Fixture::new();
    let before = std::fs::read(fixture.directory.join("owner.rrbin")).unwrap();
    let serial =
        routed_campaign_with_progress(fixture.request.clone(), &AtomicBool::new(false), |_| {})
            .unwrap();
    let mut parallel_request = fixture.request.clone();
    parallel_request.workers = 2;
    let parallel =
        routed_campaign_with_progress(parallel_request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(serial.completed_finite_trace && parallel.completed_finite_trace);
    for field in [
        "reachable_integrals",
        "rule_applications",
        "declared_terminals",
        "missing_owners",
        "missing_rules",
        "scheduled_nodes",
    ] {
        assert_eq!(
            serial.document["snapshot"][field], parallel.document["snapshot"][field],
            "{field}"
        );
    }
    assert!(
        serial.document["snapshot"]["deduplication_hits"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(serial.document["family_closure_claim"], false);
    assert_eq!(serial.document["work_checkpoint"], false);
    assert_eq!(
        std::fs::read(fixture.directory.join("owner.rrbin")).unwrap(),
        before
    );
}

#[test]
fn shared_owner_campaign_cancel_and_tiny_budget_are_incomplete() {
    let fixture = Fixture::new();
    let stopped =
        routed_campaign_with_progress(fixture.request.clone(), &AtomicBool::new(true), |_| {})
            .unwrap();
    assert!(!stopped.completed_finite_trace);
    let mut limited = fixture.request.clone();
    limited.trace_limits.max_unique_nodes = 1;
    let failed = routed_campaign_with_progress(limited, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!failed.completed_finite_trace);
    assert_eq!(failed.document["status"], "incomplete");
    assert!(failed.document["error"].as_str().is_some());
}

#[test]
fn shared_owner_campaign_admits_all_steering_before_native_load() {
    let fixture = Fixture::new();
    let mut broken = fixture.request.clone();
    // Invalid CSV wins before trying to open the deliberately missing bundle.
    broken.owner_base = fixture.directory.join("missing");
    broken.targets_csv = "trace,1".into();
    let error = routed_campaign_with_progress(broken, &AtomicBool::new(false), |_| {}).unwrap_err();
    assert!(error.to_string().contains("target CSV"));
    let mut selection: Value = serde_json::from_str(&fixture.request.selection_json).unwrap();
    selection["load_limits"] = json!({"max_total_input_bytes":1});
    assert!(
        input::Selection::parse(&selection.to_string())
            .unwrap_err()
            .to_string()
            .contains("ingress")
    );
    selection["load_limits"] = json!({"max_bundle_bytes":2usize*1024*1024*1024});
    assert!(input::Selection::parse(&selection.to_string()).is_err());
    selection["load_limits"] = json!({"max_bundle_bytes":true});
    assert!(input::Selection::parse(&selection.to_string()).is_err());
    selection["load_limits"] = json!({});
    selection["initial_frontier_routes"] = json!([{"source_mask":"1","owner_mask":"1","requires_transport":true,
        "source_to_representative":[["1","0"]],"owner_to_representative":[["1"]]}]);
    assert!(input::Selection::parse(&selection.to_string()).is_err());
}

#[test]
fn shared_owner_campaign_composes_noninvolutive_native_map_and_rejects_forgery() {
    let fixture = Fixture::new();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="generic_two_loop_route_test"
loop_momenta=["q1","q2"]
external_momenta=[]
dimension="d"
[[family.denominators]]
id="P1"
expression="q1^2-1"
[[family.denominators]]
id="P2"
expression="q2^2-1"
[[family.denominators]]
id="P3"
expression="(q1-q2)^2-1"
[target]
powers=[0,1,1]
"#;
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.nonpositive_indices = vec![0];
    generation.numerical_depth = 0;
    generation.max_numerator_rank = Some(2);
    let bundle = family_candidates(generation).unwrap();
    let inspection =
        inspect_generated_candidate_bundle(bundle.bundle(), Default::default()).unwrap();
    assert_eq!(inspection.solved_sectors, 1);
    std::fs::write(fixture.directory.join("two_loop.rrbin"), bundle.bundle()).unwrap();
    let mut selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"two_loop.rrbin","bytes":bundle.bundle().len(),"mask":"011"}],
        "initial_frontier_routes":[{"source_mask":"110","owner_mask":"011","requires_transport":true,
            "source_to_representative":[["1","0"],["0","1"]],
            "owner_to_representative":[["1","-1"],["1","0"]]}]});
    let mut request = RoutedCampaignRequest::new(selection.to_string(), "2,2,0\n".into());
    request.owner_base = fixture.directory.clone();
    let result =
        routed_campaign_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.completed_finite_trace, "{:?}", result.document);
    assert!(
        result.document["snapshot"]["transport_calls"]
            .as_u64()
            .unwrap()
            > 0
    );
    // Per-native-call and aggregate allowances are independent. A genuine
    // nonidentity numerator map must report the narrow native budget first;
    // an explicit sufficient native policy then permits the same exact trace.
    let mut ranked = request.clone();
    ranked.targets_csv = "2,2,-1\n".into();
    ranked
        .trace_limits
        .expansion
        .max_native_polynomial_operations = 1;
    let limited =
        routed_campaign_with_progress(ranked.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!limited.completed_finite_trace);
    assert!(
        limited.document["error"]
            .as_str()
            .unwrap()
            .contains("multi-affine native polynomial operations"),
        "{}",
        limited.document
    );
    ranked.trace_limits.expansion = Default::default();
    let enough =
        routed_campaign_with_progress(ranked.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(enough.completed_finite_trace, "{}", enough.document);
    ranked.trace_limits.max_transport_operations = 1;
    let aggregate = routed_campaign_with_progress(ranked, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(!aggregate.completed_finite_trace);
    assert!(
        aggregate.document["error"]
            .as_str()
            .unwrap()
            .contains("aggregate routed operations"),
        "{}",
        aggregate.document
    );
    selection["initial_frontier_routes"][0]["owner_to_representative"] =
        json!([["1", "0"], ["0", "0"]]);
    request.selection_json = selection.to_string();
    assert!(
        routed_campaign_with_progress(request.clone(), &AtomicBool::new(false), |_| {})
            .unwrap_err()
            .to_string()
            .contains("inverse")
    );
    selection["initial_frontier_routes"][0]["owner_to_representative"] =
        json!([["1", "0"], ["0", "1"]]);
    request.selection_json = selection.to_string();
    assert!(routed_campaign_with_progress(request, &AtomicBool::new(false), |_| {}).is_err());
}
