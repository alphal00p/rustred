use super::*;

fn bounded_request(fixture: &Fixture, rank: u32) -> OwnerDomainWalkRequest {
    let mut request = route_walk_request(fixture, rank);
    request.matching.queries_json = json!({
        "schema":"rustred.owner-domain-queries.json.v1", "queries":[{
            "id":"bounded-route", "owner":"110", "lower":[2,3,0],
            "upper":[4,5,0], "max_numerator_rank":rank
        }]
    })
    .to_string();
    request
}

fn without_seconds(mut value: Value) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(o) => {
                o.remove("seconds");
                for v in o.values_mut() {
                    strip(v);
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
fn bounded_initial_route_keeps_boxes_and_worker_independent_reduction_graph() {
    let fixture = noninvolutive_route_fixture();
    let request = bounded_request(&fixture, 0);
    let serial =
        owner_domain_walk_with_progress(request.clone(), &AtomicBool::new(false), |_| {}).unwrap();
    assert!(serial.all_scheduled_domains_resolved, "{}", serial.document);
    let first = &serial.document["domains"][0];
    assert_eq!(first["phase"], "Route");
    assert_eq!(first["lower"], json!([2, 3, 0]));
    assert_eq!(first["upper"], json!([4, 5, 0]));
    let mapped = serial.document["domains"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["phase"] == "Apply" && d["owner"] == "011")
        .unwrap();
    assert_eq!(mapped["lower"], json!([0, 0, 0]));
    assert_eq!(mapped["upper"], json!([null, 4, 5]));
    assert_eq!(mapped["rank"], 0);
    assert_eq!(serial.document["family_closure_claim"], false);
    assert_eq!(serial.document["routing_expanded"], false);
    for workers in [2, 6] {
        rustred::campaign::ParallelExecution::preflight_requested_core_budget(workers).unwrap();
        let mut parallel_request = request.clone();
        parallel_request.workers = workers;
        let parallel =
            owner_domain_walk_with_progress(parallel_request, &AtomicBool::new(false), |_| {})
                .unwrap();
        assert!(
            parallel.all_scheduled_domains_resolved,
            "{}",
            parallel.document
        );
        for field in [
            "domains",
            "inputs",
            "events",
            "frontiers",
            "successors",
            "conditional_successors",
            "routed_domains",
            "route_masks",
            "scheduled_nodes",
            "completed_nodes",
            "deduplication_hits",
            "exact_domain_hits",
            "full_orthant_hits",
            "containment_checks",
        ] {
            assert_eq!(
                without_seconds(parallel.document[field].clone()),
                without_seconds(serial.document[field].clone()),
                "{workers} workers: {field}"
            );
        }
    }
}

#[test]
fn bounded_initial_missing_route_and_source_conditions_keep_original_frontier_box() {
    for source_condition in [false, true] {
        let fixture = noninvolutive_route_fixture_with_scale(source_condition);
        let mut request = bounded_request(&fixture, 11);
        let mut selection: Value = serde_json::from_str(&request.matching.selection_json).unwrap();
        selection["initial_frontier_routes"] = json!([]);
        request.matching.selection_json = selection.to_string();
        let result =
            owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(!result.all_scheduled_domains_resolved);
        let frontier = if source_condition {
            &result.document["input_frontiers"][0]
        } else {
            &result.document["domains"][0]["frontiers"][0]
        };
        assert_eq!(frontier["lower"], json!([2, 3, 0]));
        assert_eq!(frontier["upper"], json!([4, 5, 0]));
        assert_eq!(frontier["rank"], 11);
        assert_eq!(frontier["reached_missing_rule_claim"], false);
    }
}

#[test]
fn bounded_literal_owner_preserves_exact_input_and_above_entry_rank() {
    let fixture = Fixture::new();
    let mut matching = match_request(&fixture);
    matching.queries_json = json!({"schema":"rustred.owner-domain-queries.json.v1",
        "queries":[{"id":"bounded-literal", "owner":"1", "lower":[2],
        "upper":[4], "max_numerator_rank":11}]})
    .to_string();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.route_domain_overcover = true;
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert!(result.all_scheduled_domains_resolved, "{}", result.document);
    assert_eq!(result.document["domains"][0]["phase"], "Apply");
    assert_eq!(result.document["domains"][0]["lower"], json!([2]));
    assert_eq!(result.document["domains"][0]["upper"], json!([4]));
    assert_eq!(result.document["domains"][0]["rank"], 11);
    assert_eq!(result.document["family_closure_claim"], false);
}

#[test]
fn bounded_ibp_successors_keep_boxes_when_entering_route_phase() {
    let fixture = Fixture::new();
    let source = r#"
schema="rustred.project.toml.v1"
[family]
name="generic_bounded_successor_route_test"
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
powers=[1,1,1]
"#;
    let mut generation = FamilyCandidatesRequest::new(source);
    generation.numerical_depth = 0;
    generation.max_numerator_rank = Some(2);
    let (_, _, _, lowered) = crate::application::lowering::lower_project(
        crate::application::input::prepare_input(source, generation.input_format).unwrap(),
    )
    .unwrap()
    .into_parts();
    let prepared = crate::application::candidate_bundle::preparation::prepare::<3>(
        lowered.into_family(),
        &[true; 3],
        None,
    )
    .unwrap();
    let solver = rustred::solver::SectorSolver::new(
        &prepared.sources,
        [true; 3],
        rustred::solver::SectorConfig {
            zero_sectors: prepared.zeros.clone(),
            permutation: prepared.permutation,
            ..Default::default()
        },
    )
    .unwrap();
    let solution = solver
        .solve_sector(rustred::solver::SectorSolveOptions {
            numerical_depth: generation.numerical_depth,
            max_numerator_rank: generation.max_numerator_rank,
            finite_case_policy: generation.finite_case_policy,
            finite_case_limits: generation.finite_case_limits,
            case_intersection_limits: generation.case_intersection_limits,
            ..Default::default()
        })
        .unwrap();
    let bundle = crate::encode_generated_candidate_sector(
        &generation,
        &prepared.family,
        [true; 3],
        &solution,
    )
    .unwrap();
    let inspection = inspect_generated_candidate_bundle(&bundle, Default::default()).unwrap();
    assert_eq!(inspection.solved_sectors, 1);
    std::fs::write(fixture.directory.join("top.rrbin"), &bundle).unwrap();
    // Install only the top owner so an actual native IBP pinch must enter
    // Route. Missing route metadata is intentional; this test checks admission,
    // not closure or a particular generated rule/ordering's preferred pinch.
    let selection = json!({"family_fingerprint":inspection.family_fingerprint,
        "owners":[{"path":"top.rrbin","bytes":bundle.len(),"mask":"111"}],
        "initial_frontier_routes":[]});
    let queries = json!({"schema":"rustred.owner-domain-queries.json.v1",
        "queries":[{"id":"bounded-native-parent", "owner":"111",
        "lower":[0,0,0], "upper":[2,2,2], "max_numerator_rank":0}]});
    let mut matching = OwnerDomainMatchRequest::new(selection.to_string(), queries.to_string());
    matching.owner_base = fixture.directory.clone();
    let mut request = OwnerDomainWalkRequest::new(matching);
    request.route_domain_overcover = true;
    let result = owner_domain_walk_with_progress(request, &AtomicBool::new(false), |_| {}).unwrap();
    assert_eq!(result.document["domains"][0]["phase"], "Apply");
    assert_eq!(result.document["domains"][0]["owner"], "111");
    assert!(result.document["successors"].as_u64().unwrap() > 0);
    let routed = result.document["domains"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|domain| domain["phase"] == "Route")
        .collect::<Vec<_>>();
    assert!(!routed.is_empty(), "{}", result.document);
    for domain in routed {
        assert_ne!(domain["owner"], "111");
        assert!(
            domain["upper"]
                .as_array()
                .unwrap()
                .iter()
                .all(Value::is_u64),
            "IBP successor widened to an orthant: {domain}"
        );
        assert!(
            domain["lower"]
                .as_array()
                .unwrap()
                .iter()
                .all(Value::is_u64)
        );
        // Frontier receipts must describe this same bounded child, not a
        // manufactured full orthant introduced at either dispatch boundary.
        for frontier in domain["frontiers"].as_array().unwrap() {
            if frontier["kind"] == "missing_route_cover" {
                assert_eq!(frontier["lower"], domain["lower"]);
                assert_eq!(frontier["upper"], domain["upper"]);
            }
        }
    }
    assert_eq!(result.document["family_closure_claim"], false);
    assert_eq!(result.document["routing_expanded"], false);
}
