use rustred_app::{
    CandidateBundleLimits, CandidateProgramInspectionOptions, FamilyCandidatesRequest,
    family_candidates, inspect_generated_candidate_program,
};

const K1: &str = r#"
schema = "rustred.project.toml.v1"
[family]
name = "inspection_control"
loop_momenta = ["q"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P"
expression = "q^2-1"
[target]
powers = [1]
"#;

#[test]
fn native_inspection_exports_real_keys_and_marks_all_omissions() {
    let generated = family_candidates(FamilyCandidatesRequest::new(K1)).unwrap();
    let report = inspect_generated_candidate_program(
        generated.bundle(),
        CandidateBundleLimits::default(),
        CandidateProgramInspectionOptions::default(),
    )
    .unwrap();
    assert_eq!(report.arity, 1);
    assert_eq!(report.total_sectors, 1);
    assert_eq!(report.omitted_rules, 0);
    assert!(!report.source_replay_claim && !report.closure_claim);
    assert_eq!(report.sectors[0].terminals, vec![vec![1]]);
    assert!(report.total_rules > 0);
    let mut options = CandidateProgramInspectionOptions::default();
    options.rule_ordinals = Some(vec![0]);
    options.include_rhs_coefficients = true;
    let detailed = inspect_generated_candidate_program(
        generated.bundle(),
        CandidateBundleLimits::default(),
        options.clone(),
    )
    .unwrap();
    assert_eq!(detailed.sectors[0].rules.len(), 1);
    assert_eq!(detailed.omitted_rules, detailed.total_rules - 1);
    assert!(detailed.sectors[0].rules[0].rhs.iter().all(|term| {
        detailed
            .coefficients
            .iter()
            .any(|c| c.id == term.coefficient_id)
    }));
    let json: serde_json::Value = serde_json::from_str(&detailed.to_json().unwrap()).unwrap();
    assert_eq!(json["rhs_coefficient_details_included"], true);
    options.max_output_bytes = 1;
    assert!(
        inspect_generated_candidate_program(
            generated.bundle(),
            CandidateBundleLimits::default(),
            options
        )
        .is_err()
    );
}

#[test]
fn native_inspection_does_not_reinterpret_priority_slots_as_coordinates() {
    let source = r#"
schema = "rustred.project.toml.v1"
[family]
name = "inspection_priority_control"
loop_momenta = ["q1", "q2"]
external_momenta = []
dimension = "d"
[[family.denominators]]
id = "P1"
expression = "q1^2-1"
[[family.denominators]]
id = "P2"
expression = "q2^2-1"
[[family.denominators]]
id = "P3"
expression = "(q1-q2)^2-1"
[target]
powers = [1,1,1]
"#;
    let mut request = FamilyCandidatesRequest::new(source);
    request.permutation = Some(vec![2, 0, 1]);
    request.selected_sectors = Some(vec![vec![true, true, false]]);
    let generated = family_candidates(request).unwrap();
    let report = inspect_generated_candidate_program(
        generated.bundle(),
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(report.priority_slots, Some(vec![2, 0, 1]));
    assert_eq!(report.sectors[0].sector, vec![true, true, false]);
    assert!(!report.sectors[0].rules.is_empty());
    assert!(!report.sectors[0].terminals.is_empty());
    assert!(
        report.sectors[0]
            .terminals
            .iter()
            .all(|key| key[0] > 0 && key[1] > 0 && key[2] <= 0)
    );
    assert!(
        report.sectors[0]
            .rules
            .iter()
            .all(|rule| rule.target.values.len() == 3)
    );
    let mut options = CandidateProgramInspectionOptions::default();
    options.sectors = Some(vec![vec![false, true, true]]);
    assert!(
        inspect_generated_candidate_program(generated.bundle(), Default::default(), options)
            .is_err()
    );
}
